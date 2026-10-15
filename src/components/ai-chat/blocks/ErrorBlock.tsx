export function ErrorBlock({ message }: { message: string }) {
  return <p className="my-2 whitespace-pre-wrap break-words rounded-lg border border-accent-error/30 bg-accent-error/10 p-3 text-xs text-accent-error">{message}</p>;
}
