type Confirmation = {
  title: string;
  description?: string;
  action: string;
  destructive?: boolean;
};

export const confirmation = $state<{
  current: (Confirmation & { resolve: (accepted: boolean) => void }) | null;
}>({ current: null });

export function confirm(options: Confirmation): Promise<boolean> {
  if (confirmation.current) return Promise.resolve(false);
  return new Promise(resolve => {
    confirmation.current = { ...options, resolve };
  });
}

export function settleConfirmation(accepted: boolean) {
  const pending = confirmation.current;
  confirmation.current = null;
  pending?.resolve(accepted);
}
