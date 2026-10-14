/**
 * Example window: the group pool.
 *
 * Shows the pattern every pool follows — read the show with `useShow`, list a
 * collection with `listOf`, and act by running a line, exactly what the
 * operator would type.
 */

import { listOf, useRunLine, useShow } from "../../core";

export function GroupsWindow() {
  const show = useShow();
  const run = useRunLine();
  const groups = show === null ? [] : listOf(show.groups);

  if (groups.length === 0) {
    return <p className="dim">No groups. Select fixtures and type <code>Store Group 1</code>.</p>;
  }
  return (
    <div className="pool">
      {groups.map((group) => (
        <button
          key={group.id}
          type="button"
          className="pool-tile"
          onClick={() => run(`Group ${String(group.id)}`)}
        >
          <span className="pool-number">{group.id}</span>
          <span className="pool-name">{group.name}</span>
          <span className="dim">{group.fixtures.length} fixtures</span>
        </button>
      ))}
    </div>
  );
}
