import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText } from "../../lib/api";
import { Modal } from "../../components/ui";

type Pending = { workflowId: string; message: string };

/** Confirmation is shown in every PulseLauncher window. Answering in one window
 * closes the prompt in the others. */
export function ConfirmPrompt({ onNotice }: { onNotice?: (title: string, body: string) => void }) {
  const [pending, setPending] = useState<Pending | null>(null);

  useEffect(() => {
    const subs = [
      listen<Pending>("workflow-confirm", (event) => setPending(event.payload)).catch(() => () => undefined),
      listen<{ workflowId: string }>("workflow-confirm-settled", (event) => {
        setPending((current) => (current?.workflowId === event.payload.workflowId ? null : current));
      }).catch(() => () => undefined),
    ];
    return () => {
      subs.forEach((pendingUnlisten) => void pendingUnlisten.then((unlisten) => unlisten()));
    };
  }, []);

  if (!pending) return null;

  async function answer(accepted: boolean) {
    if (!pending) return;
    try {
      await api.confirmWorkflow(pending.workflowId, accepted);
    } catch (error) {
      const message = errorText(error);
      if (!message.includes("not waiting")) onNotice?.("Confirm", message);
    } finally {
      setPending(null);
    }
  }

  return (
    <Modal title="Confirm this step" onClose={() => void answer(false)}>
      <p>{pending.message}</p>
      <div className="mt-4 flex justify-end gap-2">
        <button className="btn" onClick={() => void answer(false)}>Decline</button>
        <button className="btn btn-primary" onClick={() => void answer(true)}>Continue</button>
      </div>
    </Modal>
  );
}
