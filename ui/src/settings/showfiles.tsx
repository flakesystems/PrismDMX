/**
 * The Show files panel: save, save as, open, recent, autosave — S37.
 *
 * # Why this panel needed a protocol change and the other three did not
 *
 * Outputs had S33's four commands, Devices had S36's one, and *This machine* had
 * `MachineConfig`. This one had `SaveShow` and nothing else: a show could be
 * written and never opened, renamed or replaced from an interface, so the file
 * an operator was working in was whatever `--show` had named at start-up. S37
 * added `OpenShow`, `SaveShowAs`, `NewShow`, `ExportShow` and `ImportShow`.
 *
 * # None of these is a command-line word
 *
 * `ARCHITECTURE_SPEC.md` §4.5 makes the command line *the* interface and lists
 * the exceptions — the ones a line cannot express. These five are in that
 * company: S40's vocabulary has no noun for a file, exactly as it has none for a
 * window type, so this panel sends commands the way the canvas sends
 * `OpenWindow`. A path is not a word an operator types into a console line.
 *
 * # The path is typed, and — since S29 — it can also be picked
 *
 * A browser cannot show a file dialogue that names a path on the *daemon's*
 * machine — the two may not even be the same machine — so what an operator types
 * is a path the daemon resolves: absolute as given, relative against its data
 * directory. **The desktop shell offers a real dialogue and puts the result in
 * this box** (punch-list B31), and the box is what makes the panel work in a
 * browser, which is where the Web Remote (S31) will always be.
 *
 * So the *Browse…* button is drawn only where there is something behind it —
 * `shell/bridge.ts`'s {@link inShell} — and nothing else about this panel
 * changes. Nothing about the protocol changes either: what is sent is the same
 * `OpenShow`, `SaveShowAs`, `NewShow`, `ExportShow` and `ImportShow` carrying
 * the same `path`, whether a person typed it or picked it.
 */

import { useCallback, useState } from "react";

import { choosePath, inShell } from "../shell/bridge";
import { useDesk, useSend } from "../store/hooks";
import type { DeskState } from "../store/desk";
import { fileName } from "./settings";

const selectShowFile = (state: DeskState) => state.showFile;

/** What the operator is being asked for. */
type Asking = "SaveShowAs" | "OpenShow" | "NewShow" | "ExportShow" | "ImportShow";

/** What each of them says on its button and in its prompt. */
const ASKS: Record<Asking, { readonly verb: string; readonly hint: string }> = {
  SaveShowAs: {
    verb: "Save as",
    hint: "Where to write it. The show that is open from then on is this one.",
  },
  OpenShow: {
    verb: "Open",
    hint: "A .prism file that is there. An open never creates one — a typo would otherwise become an empty show beside the file you meant.",
  },
  NewShow: {
    verb: "New",
    hint: "Where to make an empty show. A name something is already at is refused rather than replaced.",
  },
  ExportShow: {
    verb: "Export JSON",
    hint: "A copy in a second format, for a diff or a backup. Nothing about the show changes, the open file included.",
  },
  ImportShow: {
    verb: "Import JSON",
    hint: "Read an export back over the running show. It is not written to disk, so the Save lamp stays lit until you save.",
  },
};

/** The whole panel. */
export function ShowFilesPanel() {
  const showFile = useDesk(selectShowFile);
  const send = useSend();
  const [asking, setAsking] = useState<Asking | null>(null);
  const [path, setPath] = useState("");

  const open = useCallback((what: Asking) => {
    setAsking(what);
    setPath("");
  }, []);

  /**
   * The operating system's own dialogue, for the row that is open.
   *
   * It fills the box rather than sending the command, deliberately: the form
   * still has its Apply button, so a path picked by mistake is corrected the
   * same way a path typed by mistake is, and the hint under the box — which is
   * the only place *Open never creates one* is written down — is still read
   * before anything happens.
   */
  const browse = useCallback(() => {
    if (asking === null) {
      return;
    }
    void choosePath(asking, path.trim() === "" ? showFile?.path : path.trim()).then((chosen) => {
      if (chosen !== null) {
        setPath(chosen);
      }
    });
  }, [asking, path, showFile]);

  const submit = useCallback(() => {
    if (asking === null || path.trim() === "") {
      return;
    }
    send({ t: asking, path: path.trim() });
    // Dropped the moment it is sent, like every other draft in this interface:
    // what file is open comes back as `ShowFileChanged`.
    setAsking(null);
    setPath("");
  }, [asking, path, send]);

  if (showFile === null) {
    return <p className="window-note">The daemon has not said which show it is holding.</p>;
  }

  return (
    <div className="settings-panel" data-testid="settings-showfiles">
      <dl className="status-strip" data-testid="show-readings">
        <div className="reading">
          <dt>Open</dt>
          <dd data-testid="show-path" title={showFile.path}>
            {fileName(showFile.path)}
          </dd>
        </div>
        <div className="reading">
          <dt>State</dt>
          <dd data-testid="show-dirty">
            {showFile.unsavedChanges ? "unsaved changes" : "saved"}
          </dd>
        </div>
        <div className="reading">
          <dt>Autosave</dt>
          <dd data-testid="show-autosave">
            {showFile.recovery
              ? `a recovery copy is standing, written every ${String(showFile.autosaveSeconds)} s`
              : `every ${String(showFile.autosaveSeconds)} s while there is anything unsaved`}
          </dd>
        </div>
      </dl>
      <p className="settings-hint" data-testid="show-full-path">
        {showFile.path}
      </p>
      <div className="settings-bar">
        <button
          type="button"
          data-testid="show-save"
          onClick={() => {
            send({ t: "SaveShow" });
          }}
        >
          Save
        </button>
        {(Object.keys(ASKS) as Asking[]).map((what) => (
          <button
            key={what}
            type="button"
            data-testid={`show-${what}`}
            onClick={() => {
              open(what);
            }}
          >
            {ASKS[what].verb}
          </button>
        ))}
      </div>
      {asking === null ? null : (
        <form
          className="settings-form"
          data-testid="show-form"
          onSubmit={(event) => {
            event.preventDefault();
            submit();
          }}
        >
          <fieldset>
            <legend>{ASKS[asking].verb}</legend>
            <label>
              Path
              <input
                data-testid="show-path-input"
                value={path}
                placeholder={
                  asking === "ExportShow" || asking === "ImportShow"
                    ? "aula.json"
                    : "aula.prism"
                }
                onChange={(event) => {
                  setPath(event.target.value);
                }}
              />
            </label>
            {inShell() ? (
              <button type="button" data-testid="show-form-browse" onClick={browse}>
                Browse&hellip;
              </button>
            ) : null}
          </fieldset>
          <p className="settings-hint" data-testid="show-form-hint">
            {ASKS[asking].hint} A relative name is taken as being in the desk&rsquo;s own data
            directory.
          </p>
          <div className="settings-actions">
            <button type="submit" data-testid="show-form-apply" disabled={path.trim() === ""}>
              {ASKS[asking].verb}
            </button>
            <button
              type="button"
              data-testid="show-form-cancel"
              onClick={() => {
                setAsking(null);
              }}
            >
              Cancel
            </button>
          </div>
        </form>
      )}
      <RecentShows paths={showFile.recent} />
    </div>
  );
}

/**
 * The shows this desk has had open, most recent first.
 *
 * **Picked out of a list, so it is sent at once** — `ARCHITECTURE_SPEC.md` §4.5's
 * rule about an item chosen from a list: the pointer has supplied the argument,
 * so there is nothing left to type.
 */
function RecentShows({ paths }: { readonly paths: readonly string[] }) {
  const send = useSend();
  if (paths.length === 0) {
    return null;
  }
  return (
    <section data-testid="show-recent">
      <h3>Recent</h3>
      <ul className="library-matches">
        {paths.map((path) => (
          <li key={path}>
            <button
              type="button"
              className="linkish"
              title={path}
              data-testid={`show-recent-${fileName(path)}`}
              onClick={() => {
                send({ t: "OpenShow", path });
              }}
            >
              {fileName(path)}
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
