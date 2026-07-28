import { useEffect, useRef, type KeyboardEvent, type ReactNode } from "react";
import {
  BookOpenCheck,
  Database,
  ExternalLink,
  FileCheck2,
  ShieldCheck,
  Sparkles,
  X,
} from "lucide-react";
import { ExternalUrlLink } from "./ExternalUrlLink";
import "./CreditsPanel.css";

interface CreditsPanelProps {
  onClose: () => void;
}

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

export function CreditsPanel({ onClose }: CreditsPanelProps) {
  const panelRef = useRef<HTMLElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(
    typeof document !== "undefined" && document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );

  useEffect(() => {
    closeButtonRef.current?.focus();
    return () => {
      returnFocusRef.current?.focus();
    };
  }, []);

  const handleKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== "Tab") return;

    const focusable = [...(panelRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR) ?? [])]
      .filter((element) => !element.hasAttribute("hidden") && element.getAttribute("aria-hidden") !== "true");
    if (!focusable.length) {
      event.preventDefault();
      panelRef.current?.focus();
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    } else if (!panelRef.current?.contains(document.activeElement)) {
      event.preventDefault();
      first.focus();
    }
  };

  return (
    <div
      className="modal-backdrop credits-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <aside
        ref={panelRef}
        className="side-panel credits-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby="credits-panel-title"
        aria-describedby="credits-panel-description"
        tabIndex={-1}
        onKeyDown={handleKeyDown}
      >
        <div className="panel-header">
          <div>
            <span className="eyebrow">Provenance & notices</span>
            <h2 id="credits-panel-title">Credits & data sources</h2>
          </div>
          <button
            ref={closeButtonRef}
            className="icon-button"
            onClick={onClose}
            aria-label="Close credits and data sources"
            type="button"
          >
            <X size={19} />
          </button>
        </div>

        <div className="panel-body credits-body">
          <p id="credits-panel-description" className="credits-intro">
            Analysis runs locally. Automated data requests occur only through the app&apos;s
            disclosed import or update actions. External credit links open only when selected, and
            attribution does not imply endorsement.
          </p>

          <section className="credit-source-card">
            <div className="credit-source-icon official" aria-hidden="true">
              <BookOpenCheck size={19} />
            </div>
            <div>
              <div className="credit-source-heading">
                <h3>Wizards of the Coast</h3>
                <span>Official rules & policy</span>
              </div>
              <p>
                Magic card names, Oracle text, the Comprehensive Rules, Commander format rules,
                and published Commander policy originate with Wizards of the Coast.
              </p>
              <div className="credit-links">
                <ExternalCreditLink href="https://magic.wizards.com/en/rules">
                  Comprehensive Rules
                </ExternalCreditLink>
                <ExternalCreditLink href="https://magic.wizards.com/en/formats/commander">
                  Commander format
                </ExternalCreditLink>
              </div>
            </div>
          </section>

          <section className="credit-source-card">
            <div className="credit-source-icon imports" aria-hidden="true">
              <FileCheck2 size={19} />
            </div>
            <div>
              <div className="credit-source-heading">
                <h3>Deck import providers</h3>
                <span>User-requested deck retrieval</span>
              </div>
              <p>
                Direct URL imports contact Archidekt, Deckstats, or Scryfall Decks only after the
                user submits that provider&apos;s public deck URL. Moxfield is recognized as an
                export workflow only; the app does not use its non-public API or bypass challenge
                pages.
              </p>
              <div className="credit-links">
                <ExternalCreditLink href="https://archidekt.com/">Visit Archidekt</ExternalCreditLink>
                <ExternalCreditLink href="https://deckstats.net/">Visit Deckstats</ExternalCreditLink>
                <ExternalCreditLink href="https://moxfield.com/">Visit Moxfield</ExternalCreditLink>
              </div>
            </div>
          </section>

          <section className="credit-source-card">
            <div className="credit-source-icon scryfall" aria-hidden="true">
              <Database size={19} />
            </div>
            <div>
              <div className="credit-source-heading">
                <h3>Scryfall</h3>
                <span>Card & bulk data</span>
              </div>
              <p>
                Scryfall supplies card identities, current Oracle information, images, and optional
                bulk card-data updates. The app keeps its installed snapshot and analysis local.
              </p>
              <div className="credit-links">
                <ExternalCreditLink href="https://scryfall.com/">Visit Scryfall</ExternalCreditLink>
                <ExternalCreditLink href="https://scryfall.com/docs/api/bulk-data">
                  Bulk data documentation
                </ExternalCreditLink>
              </div>
            </div>
          </section>

          <section className="credit-source-card">
            <div className="credit-source-icon spellbook" aria-hidden="true">
              <Sparkles size={19} />
            </div>
            <div>
              <div className="credit-source-heading">
                <h3>Commander Spellbook</h3>
                <span>Optional combo catalog</span>
              </div>
              <p>
                A user-selected update can install Commander Spellbook&apos;s documented combo
                catalog for local matching. Catalog entries remain attributed and do not become
                rules-engine proof merely because a line is documented.
              </p>
              <div className="credit-links">
                <ExternalCreditLink href="https://commanderspellbook.com/">
                  Visit Commander Spellbook
                </ExternalCreditLink>
              </div>
            </div>
          </section>

          <section className="credits-notice-card" aria-labelledby="open-source-heading">
            <ShieldCheck size={19} aria-hidden="true" />
            <div>
              <h3 id="open-source-heading">Open-source software</h3>
              <p>
                This application includes open-source libraries. Project and service attribution
                is included with the app in <strong>THIRD_PARTY_NOTICES.md</strong>.
              </p>
            </div>
          </section>

          <section className="unofficial-notice" aria-label="Unofficial fan content notice">
            <strong>Unofficial and independent</strong>
            <p>
              Commander Deck Analyzer is unofficial Fan Content permitted under the Fan Content
              Policy. Not approved or endorsed by Wizards. Portions of the materials used are
              property of Wizards of the Coast. © Wizards of the Coast LLC. This app is not
              affiliated with Scryfall, Commander Spellbook, or Moxfield.
            </p>
          </section>
        </div>

        <div className="panel-footer">
          <button className="compact-primary full" onClick={onClose} type="button">
            Done
          </button>
        </div>
      </aside>
    </div>
  );
}

function ExternalCreditLink({
  href,
  children,
}: {
  href: string;
  children: ReactNode;
}) {
  return (
    <ExternalUrlLink href={href}>
      <span>{children}</span>
      <ExternalLink size={12} aria-hidden="true" />
      <span className="sr-only">(opens in a new tab)</span>
    </ExternalUrlLink>
  );
}
