import { X } from "lucide-react";
import { observer } from "mobx-react-lite";
import { confirmation,settleConfirmation } from "./confirmation";
import {
AlertDialog,
AlertDialogAction,
AlertDialogCancel,
AlertDialogContent,
AlertDialogDescription,
AlertDialogTitle,
} from "./shadcn/alert-dialog";
import { Button } from "./ui";

export default observer(function ConfirmDialog() {
  const current = confirmation.current;
  return (
    current && (
      <AlertDialog
        open
        onOpenChange={(open) => {
          if (!open) settleConfirmation(false);
        }}
      >
        <AlertDialogContent className="modal reponiq-dialog">
          <AlertDialogTitle>{current.title}</AlertDialogTitle>
          <AlertDialogDescription
            className={current.description ? undefined : "sr-only"}
          >
            {current.description ?? current.title}
          </AlertDialogDescription>
          <div className="modal-actions">
            <AlertDialogCancel asChild>
              <Button variant="secondary">Cancel</Button>
            </AlertDialogCancel>
            <AlertDialogAction asChild onClick={() => settleConfirmation(true)}>
              <Button variant={current.destructive ? "destructive" : "primary"}>
                {current.action}
              </Button>
            </AlertDialogAction>
          </div>
          <Button
            className="dialog-close"
            aria-label="Close dialog"
            title="Close dialog"
            onClick={() => settleConfirmation(false)}
          >
            <X size={18} />
          </Button>
        </AlertDialogContent>
      </AlertDialog>
    )
  );
});
