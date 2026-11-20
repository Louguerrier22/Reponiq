import { X } from "lucide-react";
import { useRef,type PropsWithChildren } from "react";
import { Dialog,DialogContent,DialogTitle } from "../shadcn/dialog";
import { Button } from "../ui";

export default function Modal({
  label,
  onclose,
  canClose = true,
  walkthrough = false,
  children,
}: PropsWithChildren<{
  label: string;
  onclose: () => void;
  canClose?: boolean;
  walkthrough?: boolean;
}>) {
  const opener = useRef(
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && canClose) onclose();
      }}
    >
      <DialogContent
        className={
          walkthrough ? "tour-overlay reponiq-dialog" : "modal reponiq-dialog"
        }
        showCloseButton={false}
        aria-describedby={undefined}
        onInteractOutside={(event) => event.preventDefault()}
        onEscapeKeyDown={(event) => {
          if (!canClose) event.preventDefault();
        }}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          (opener.current?.isConnected
            ? opener.current
            : document.getElementById("main-content")
          )?.focus({ preventScroll: true });
        }}
      >
        <DialogTitle className="sr-only">{label}</DialogTitle>
        {children}
        {!walkthrough && (
          <Button
            className="dialog-close"
            aria-label="Close dialog"
            title="Close dialog"
            disabled={!canClose}
            onClick={onclose}
          >
            <X size={18} />
          </Button>
        )}
      </DialogContent>
    </Dialog>
  );
}
