/**
 * The `CommandKeys` window: the console's words, as buttons — S43, B12.
 *
 * They were a keypad under the command line until this session, where the
 * owner's punch list found them in the way: the line is the interface
 * (`ARCHITECTURE_SPEC.md` §4.5), and nineteen buttons crowding it made it look
 * like the smaller half. In a window they can be opened by somebody learning
 * the vocabulary and closed by somebody who has learned it, which is what the
 * entry asks for in as many words — *otherwise one types, or works the buttons
 * on the surface*.
 *
 * # Nothing here sends a command of its own
 *
 * A `run` key writes its word and submits, a `write` key writes it and waits, an
 * `append` key adds it to what is there; the line decides the rest, exactly as
 * it does for a typed one. That is `docs/COMMAND_LINE.md` §1 and it is the
 * reason this window teaches the vocabulary rather than replacing it.
 *
 * # And every key hands the keyboard back to the line
 *
 * `Store ` and `Cue ` are lines **waiting for a number**, and the number comes
 * from the keyboard — so a key that left the focus on itself would make every
 * gesture two gestures: press the word, then find the input. Pressing a key
 * therefore focuses the command line, which is the same place the operator's
 * hands were going anyway.
 *
 * `run` keys do it too. `Clear` acts at once and takes no argument, but what an
 * operator does *after* clearing is type, and a key that had to be un-focused
 * first would be the one key on the pad that behaved differently.
 *
 * # The Update key blinks here — B62
 *
 * It blinked on the Cue Viewer's store bar until that bar went (cues are stored
 * from the command line). A console has its Update key on the keypad, so that is
 * where the blink went: `Session::editingCue.modified`, the daemon's state, so
 * every screen's key blinks together — and the title names the cue it would
 * write back into. The key still writes `Update` and runs it; it carries
 * nothing, because which cue and which mode are the desk's.
 */

import type { JsonValue } from "../bindings";
import { cueEditInForce } from "../show/looks";
import { useConsole } from "./consoleshell";
import { COMMAND_INPUT_ID, focusCommandLine } from "./commandinput";
import { CONSOLE_KEYS, titleOf } from "./keys";

/** What the keypad reads: the session, for the Update key's blink. */
export interface KeypadProps {
  /** The session document, or nothing before there is one. */
  readonly session: JsonValue | null;
}

/** The keypad. */
export function Keypad({ session }: KeypadProps) {
  const { write, append, run, oops } = useConsole();
  const editing = cueEditInForce(session);
  return (
    <div className="command-keys" data-testid="command-keys">
      {CONSOLE_KEYS.map((key) => (
        <button
          key={key.word}
          type="button"
          className={`command-key command-key-${key.shape}${updateClass(key.word, editing)}`}
          data-testid={`key-${key.word.toLowerCase()}`}
          data-shape={key.shape}
          data-modified={key.word === "Update" && editing?.modified === true ? "yes" : "no"}
          title={
            key.word === "Update" && editing !== null
              ? `Store the programmer back into cue ${editing.cueNumber}`
              : titleOf(key.word)
          }
          // The line is somewhere else on the screen — this is a window and the
          // line is a band — so the key names it by its id rather than holding a
          // ref to it. `COMMAND_INPUT_ID` is exported from the component that
          // draws it, so the two cannot drift apart silently.
          aria-controls={COMMAND_INPUT_ID}
          onClick={() => {
            if (key.shape === "run") {
              run(key.word);
            } else if (key.shape === "oops") {
              oops();
            } else if (key.shape === "write") {
              write(`${key.word} `);
            } else {
              append(key.word);
            }
            focusCommandLine();
          }}
        >
          {key.word}
        </button>
      ))}
    </div>
  );
}

/** The Update key's extra classes: an Update key, blinking while there is a change to put back. */
function updateClass(word: string, editing: ReturnType<typeof cueEditInForce>): string {
  if (word !== "Update" || editing === null) {
    return "";
  }
  return editing.modified ? " update-key update-blinking" : " update-key";
}
