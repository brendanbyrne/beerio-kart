import { useFormStatus } from 'react-dom';
import { clsx } from 'clsx';
import type { ReactNode } from 'react';

type SubmitButtonProps = {
  children: ReactNode;
  pendingLabel?: ReactNode;
  className?: string;
};

export function SubmitButton({
  children,
  pendingLabel,
  className,
}: SubmitButtonProps) {
  const { pending } = useFormStatus();
  return (
    <button
      type="submit"
      disabled={pending}
      className={clsx(
        'disabled:bg-gray-300 disabled:cursor-not-allowed',
        className,
      )}
    >
      {pending && pendingLabel !== undefined ? pendingLabel : children}
    </button>
  );
}
