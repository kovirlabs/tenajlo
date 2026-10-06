type Props = { details: string | null };

/** Raw technical output, hidden behind a "Details" toggle (spec §8.2). */
export function ErrorDetails({ details }: Props) {
  if (!details) return null;
  return (
    <details className="error-details">
      <summary>Details</summary>
      <pre>{details}</pre>
    </details>
  );
}
