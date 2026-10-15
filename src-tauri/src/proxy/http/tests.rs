use openssl::asn1::Asn1Time;
use openssl::bn::BigNum;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use openssl::ssl::{SslAcceptor, SslMethod};
use openssl::x509::extension::{
    BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAlternativeName,
};
use openssl::x509::{X509NameBuilder, X509};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// A disposable local CA/leaf exercises real TLS without changing OS trust.
fn tls_fixture() -> (String, Vec<u8>, JoinHandle<bool>) {
    let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_text("CN", "Tabularis disposable test CA")
        .unwrap();
    let name = name.build();
    let mut ca = X509::builder().unwrap();
    ca.set_version(2).unwrap();
    ca.set_serial_number(&BigNum::from_u32(1).unwrap().to_asn1_integer().unwrap())
        .unwrap();
    ca.set_subject_name(&name).unwrap();
    ca.set_issuer_name(&name).unwrap();
    ca.set_pubkey(&key).unwrap();
    ca.set_not_before(&Asn1Time::days_from_now(0).unwrap())
        .unwrap();
    ca.set_not_after(&Asn1Time::days_from_now(1).unwrap())
        .unwrap();
    ca.append_extension(BasicConstraints::new().critical().ca().build().unwrap())
        .unwrap();
    ca.append_extension(KeyUsage::new().key_cert_sign().build().unwrap())
        .unwrap();
    ca.sign(&key, MessageDigest::sha256()).unwrap();
    let ca = ca.build();

    let mut leaf_name = X509NameBuilder::new().unwrap();
    leaf_name.append_entry_by_text("CN", "localhost").unwrap();
    let mut leaf = X509::builder().unwrap();
    leaf.set_version(2).unwrap();
    leaf.set_serial_number(&BigNum::from_u32(2).unwrap().to_asn1_integer().unwrap())
        .unwrap();
    leaf.set_subject_name(&leaf_name.build()).unwrap();
    leaf.set_issuer_name(ca.subject_name()).unwrap();
    leaf.set_pubkey(&key).unwrap();
    leaf.set_not_before(&Asn1Time::days_from_now(0).unwrap())
        .unwrap();
    leaf.set_not_after(&Asn1Time::days_from_now(1).unwrap())
        .unwrap();
    leaf.append_extension(ExtendedKeyUsage::new().server_auth().build().unwrap())
        .unwrap();
    let san = SubjectAlternativeName::new()
        .dns("localhost")
        .build(&leaf.x509v3_context(Some(&ca), None))
        .unwrap();
    leaf.append_extension(san).unwrap();
    leaf.sign(&key, MessageDigest::sha256()).unwrap();

    let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    acceptor.set_private_key(&key).unwrap();
    acceptor.set_certificate(&leaf.build()).unwrap();
    let acceptor = acceptor.build();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(3)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_secs(3)))
                        .unwrap();
                    let mut stream = match acceptor.accept(stream) {
                        Ok(stream) => stream,
                        Err(error) => {
                            eprintln!("Disposable TLS fixture handshake failed: {error:?}");
                            return false;
                        }
                    };
                    let mut request = [0; 4096];
                    if stream.read(&mut request).is_err() {
                        return false;
                    }
                    return stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                        )
                        .is_ok();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return false,
            }
        }
        false
    });
    (
        format!("https://localhost:{port}"),
        ca.to_pem().unwrap(),
        server,
    )
}

#[tokio::test]
async fn ai_http_transport_rejects_an_untrusted_certificate() {
    let (url, _, server) = tls_fixture();
    let error = super::direct_http_client()
        .unwrap()
        .get(url)
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .unwrap_err();
    assert!(error.is_connect(), "{error:?}");
    assert!(format!("{error:?}").to_lowercase().contains("certificate"));
    assert!(!server.join().unwrap());
}

#[tokio::test]
async fn reqwest_012_transport_also_rejects_an_untrusted_certificate() {
    let (url, _, server) = tls_fixture();
    let error = super::build_reqwest_012_client(None, Duration::from_secs(3))
        .unwrap()
        .get(url)
        .send()
        .await
        .unwrap_err();
    assert!(error.is_connect(), "{error:?}");
    assert!(format!("{error:?}").to_lowercase().contains("certificate"));
    assert!(!server.join().unwrap());
}

#[tokio::test]
async fn explicit_test_ca_accepts_the_right_host_and_rejects_a_name_mismatch() {
    for correct_host in [true, false] {
        let (url, ca, server) = tls_fixture();
        let client = reqwest::Client::builder()
            .no_proxy()
            .add_root_certificate(reqwest::Certificate::from_pem(&ca).unwrap())
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        let url = if correct_host {
            url
        } else {
            url.replace("localhost", "127.0.0.1")
        };
        let result = client.get(url).send().await;
        if correct_host {
            assert_eq!(result.unwrap().text().await.unwrap(), "ok");
            assert!(server.join().unwrap());
        } else {
            let error = result.unwrap_err();
            assert!(error.is_connect(), "{error:?}");
            assert!(format!("{error:?}").to_lowercase().contains("certificate"));
            assert!(!server.join().unwrap());
        }
    }
}
