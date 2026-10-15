import { isValidElement } from "react";
import ReactMarkdown from "react-markdown";
import type { AiSqlActions } from "../../../types/aiEditor";
import { isAiSqlCode } from "../../../utils/aiChatMarkdown";
import { SqlBlock } from "./SqlBlock";

export function TextBlock({ text, actions, dialect }: { text: string; actions?: AiSqlActions; dialect?: string }) {
  return (
    <div className="break-words text-sm leading-relaxed text-primary [&_p]:my-2 [&_ul]:list-disc [&_ul]:pl-5 [&_ol]:list-decimal [&_ol]:pl-5 [&_h1]:font-semibold [&_h2]:font-semibold [&_h3]:font-semibold [&_blockquote]:border-l-2 [&_blockquote]:border-strong [&_blockquote]:pl-3 [&_a]:text-accent [&_code]:font-mono">
      <ReactMarkdown skipHtml components={{
        // Remote images are omitted: rendering a model-supplied URL would make
        // an unsolicited network request. Blob-backed images arrive in M10.
        img: () => null,
        a: ({ children, href }) => <a href={href} target="_blank" rel="noopener noreferrer" className="focus-ring">{children}</a>,
        pre: ({ children }) => {
          if (isValidElement<{ children?: unknown; className?: string }>(children)) {
            const code = children.props.children;
            const language = /language-([^\s]+)/.exec(children.props.className ?? "")?.[1] ?? "";
            if (typeof code === "string" && isAiSqlCode(language, code, dialect)) {
              return <SqlBlock sql={code.replace(/\n$/, "")} actions={actions} />;
            }
          }
          return <pre className="my-3 overflow-x-auto rounded-lg border border-default bg-base p-3 text-xs">{children}</pre>;
        },
      }}>{text}</ReactMarkdown>
    </div>
  );
}
