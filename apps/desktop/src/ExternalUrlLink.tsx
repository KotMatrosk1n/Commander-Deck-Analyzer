import type { AnchorHTMLAttributes, MouseEvent, ReactNode } from "react";
import { openExternalCreditUrl } from "./api";

interface ExternalUrlLinkProps
  extends Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href" | "target" | "rel" | "onClick"> {
  href: string;
  children: ReactNode;
}

export function ExternalUrlLink({
  href,
  children,
  ...anchorProps
}: ExternalUrlLinkProps) {
  const openFromNativeApp = (event: MouseEvent<HTMLAnchorElement>) => {
    if (!("__TAURI_INTERNALS__" in window)) return;

    event.preventDefault();
    void openExternalCreditUrl(href).catch((reason: unknown) => {
      console.error("The external attribution link could not be opened.", reason);
    });
  };

  return (
    <a
      {...anchorProps}
      href={href}
      target="_blank"
      rel="noreferrer"
      onClick={openFromNativeApp}
    >
      {children}
    </a>
  );
}
