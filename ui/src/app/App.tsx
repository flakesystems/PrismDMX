/**
 * The interface: a device screen in four fixed bands.
 *
 * ```text
 *   header      PrismDMX + connection light · View Selector Bar · Clear
 *   canvas      the windows the session says are open   (all remaining height)
 *   command     the command line — every key on the desk ends up as a line
 *   programmer  the encoder banks, the selection, what is touched
 * ```
 *
 * Same arrangement as the interface before the redesign
 * (`design/skeleton/main-layout.pdf`); the old code is in `ui/legacy/` to look
 * things up in.
 *
 * **Nothing on this screen is state this interface holds.** Which windows are
 * open, which view is lit, which bank is selected: all of it is the session
 * document, shared with every other screen and with the X-Touch. A click sends
 * a command; the daemon answers with a change; the screen follows.
 */

import { useConnected } from "../core";
import { Canvas } from "./layout/Canvas";
import { CommandLine } from "./layout/CommandLine";
import { Header } from "./layout/Header";
import { NotConnected } from "./layout/NotConnected";
import { Notices } from "./layout/Notices";
import { ProgrammerBand } from "./layout/ProgrammerBand";

export default function App() {
  const connected = useConnected();
  return (
    <main className="desk" data-testid="desk">
      <Header />
      {connected ? (
        <>
          <Canvas />
          <CommandLine />
          <ProgrammerBand />
        </>
      ) : (
        <NotConnected />
      )}
      <Notices />
    </main>
  );
}
