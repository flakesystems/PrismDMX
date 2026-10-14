/**
 * Messages for the operator — a refused command, a saved show — newest last.
 * Shown over the bottom right of the canvas and never pushing anything around.
 */

import { useNotices } from "../../core";

/** How many are shown at once; the store keeps more. */
const VISIBLE = 4;

export function Notices() {
  const notices = useNotices();
  return (
    <ol className="notices" aria-live="polite">
      {notices.slice(-VISIBLE).map((notice) => (
        <li key={notice.id} className={`notice notice-${notice.level}`}>
          {notice.message}
        </li>
      ))}
    </ol>
  );
}
