/**
 * How something that is not the command line hands the keyboard back to it.
 *
 * A module of its own, and that is not tidiness: `commandline.tsx` draws a
 * component, and a file that exports both a component and a constant loses
 * fast refresh for the whole module — the interface reloads instead of
 * updating, which is a worse thing to live with than one extra file.
 *
 * # An id rather than a ref
 *
 * The `CommandKeys` window is a *window* and the line is a *band*, so they are
 * in different subtrees of the canvas and a ref cannot reach across. An id can,
 * and one exported constant is one place for it to be wrong.
 */

/** The id of the command line's input. */
export const COMMAND_INPUT_ID = "command-input";

/**
 * Puts the keyboard back in the command line.
 *
 * **S43.** Every key of the keypad calls it: `Store ` and `Cue ` are lines
 * waiting for a number, and a key that left the focus on itself would make one
 * gesture into two. It does nothing at all when there is no line on the screen
 * — which is the state a disconnected desk is in.
 */
export function focusCommandLine(): void {
  const input = document.getElementById(COMMAND_INPUT_ID);
  if (input instanceof HTMLInputElement) {
    input.focus();
    // The caret goes to the end, because the line has just been *written* and
    // what comes next is typed onto it. Without this a browser that restored a
    // previous selection would put the number in the middle of the word.
    input.setSelectionRange(input.value.length, input.value.length);
  }
}
