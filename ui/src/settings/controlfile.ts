/**
 * A control map as a **file**: written out, and read back in — S43.
 *
 * *Die Controls sollen exportiert und importiert werden können.* Both halves are
 * here, and what travels is a **profile file** — the same document
 * `prism_surface::Bindings::parse` reads and `profiles/surface/xtouch.json` is
 * written in. That is what makes an export more than a backup: it can be handed
 * to a daemon with `--surface-profile`, named in *Devices*, or e-mailed to
 * somebody else's desk and read there.
 *
 * # Nothing here knows what a surface is
 *
 * The `device` key and the `profileVersion` are the **daemon's** answer
 * (`Answer::SurfaceBindings`), passed in. A client that wrote the strings itself
 * would be keeping a second copy of two constants, and an export written against
 * a stale copy is a file the daemon then refuses — which is the failure this
 * whole file is trying to avoid.
 *
 * # What is checked on the way in, and what is not
 *
 * Checked here: that it is JSON, that it is an object, that the version and the
 * device match this desk, and that `bindings` is a list of `{control, action}`.
 * Everything about the **actions** is left to the daemon, deliberately: it
 * refuses a reserved control and it is the only thing that knows the control
 * names, so a client that pre-validated would be a second gate with a different
 * opinion. A row naming a control this surface has not got is dropped by the
 * caller, because the caller walks the *daemon's* list of controls and looks
 * each one up.
 *
 * An action is passed through as it was read — `unknown` narrowed to *an object
 * with a `t`* and no further. That is not laxness: `SurfaceAction` is nineteen
 * variants and re-checking their shapes here would be a third copy of the
 * vocabulary, after Rust's and the generated TypeScript's. What a wrong shape
 * costs is one refused `SurfaceBinding` with the daemon's own message, which is
 * where an operator wants to read it.
 */

import type { SurfaceAction, SurfaceControl } from "../bindings";

/** How a profile file writes one row. */
interface ProfileRow {
  readonly control: string;
  readonly action: SurfaceAction | null;
}

/**
 * The table as a profile file.
 *
 * **Every control, bound or not**, exactly as `Answer::SurfaceBindings` sends
 * them: a file that left the empty keys out would be a table that *merges* when
 * it is read back, and the whole point of an export is that reading it gives the
 * desk it came from. `Bindings::parse` starts from an empty table for the same
 * reason.
 *
 * Two spaces of indentation and a trailing newline, so the file is one an
 * operator can open and edit — which `profiles/surface/xtouch.json` is, and
 * which is the shape a diff reads well in.
 */
export function profileDocument(
  deviceKey: string,
  profileVersion: number,
  controls: readonly SurfaceControl[],
): string {
  const document = {
    profileVersion,
    device: deviceKey,
    documentation:
      "Exported from the PrismDMX control editor. Read by prism_surface::Bindings::parse; " +
      "see docs/MCU_MAPPING.md section 4. The note and CC numbers are deliberately not here: " +
      "they live in prism_surface::profile, and this file maps logical controls to actions.",
    bindings: controls.map(
      (control): ProfileRow => ({ control: control.name, action: control.action }),
    ),
  };
  return `${JSON.stringify(document, null, 2)}\n`;
}

/**
 * A profile file read back, as control name → action.
 *
 * Answers a **sentence** rather than throwing when the file is not one this desk
 * can use, because the caller draws it: an operator who picked the wrong file
 * needs to be told which wrong thing it was, and *nothing happened* is the worst
 * of the possible answers.
 *
 * A control the file binds to `null` is *in the map with a null*, which is not
 * the same as being absent: absent means the file said nothing and the caller
 * unbinds it anyway (an import replaces a table rather than merging into one),
 * but the distinction is kept here so a later caller that does want to merge
 * can tell them apart.
 */
export function readProfile(
  text: string,
  deviceKey: string,
  profileVersion: number,
): Map<string, SurfaceAction | null> | string {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    return `That file is not JSON: ${error instanceof Error ? error.message : "unreadable"}.`;
  }
  if (!isRecord(parsed)) {
    return "That file does not hold a profile: the top of it is not an object.";
  }
  const version = parsed["profileVersion"];
  if (version !== profileVersion) {
    return `That profile is version ${String(version)}; this desk reads version ${String(profileVersion)}.`;
  }
  const device = parsed["device"];
  if (device !== deviceKey) {
    return `That profile is for ${typeof device === "string" ? device : "an unnamed surface"}; this desk is ${deviceKey}.`;
  }
  const rows = parsed["bindings"];
  if (!Array.isArray(rows)) {
    return "That profile has no bindings list.";
  }
  const table = new Map<string, SurfaceAction | null>();
  for (const row of rows) {
    if (!isRecord(row)) {
      continue;
    }
    const control = row["control"];
    if (typeof control !== "string") {
      continue;
    }
    table.set(control, asAction(row["action"]));
  }
  if (table.size === 0) {
    return "That profile's bindings list is empty.";
  }
  return table;
}

/**
 * An action out of a file, or `null`.
 *
 * Narrowed to *an object carrying a `t`* and no further — see the module
 * documentation for why the shapes are the daemon's to check. The cast is the
 * one place in this interface where a wire value is taken on trust, and it is
 * taken on trust **towards the daemon**, which refuses what it cannot use.
 */
function asAction(value: unknown): SurfaceAction | null {
  if (!isRecord(value) || typeof value["t"] !== "string") {
    return null;
  }
  return value as unknown as SurfaceAction;
}

/** Whether a decoded value is a JSON object. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
