/**
 * A `.gdtf` file, written for the end-to-end suite — **S60**.
 *
 * # Why the suite writes one instead of installing the library
 *
 * The desk's library is GDTF since S60, and GDTF's upstream — `gdtf-share.com`
 * — has **no anonymous download**: fetching it needs an account, which is that
 * service's decision and not this project's. So there is nothing a CI job can
 * install, and a browser test of *what a GDTF profile looks like in the patch
 * window* would either skip for ever or need a network.
 *
 * The way out is the one the desk already offers a venue: a `.gdtf` file
 * dropped into `fixtures/` inside the daemon's **data directory** is read at
 * start-up and wins its key (punch-list B43, and S60's half of it). A test can
 * write one there before the daemon starts, and then everything after that is
 * the real path — the real reader, the real daemon, the real browser.
 *
 * # It is a ZIP, written by hand
 *
 * A `.gdtf` is a ZIP archive holding a `description.xml`. Everything here is
 * **stored** rather than deflated, so this is the container's own fields and no
 * compression at all; `prism_core::library::zip` is what reads it, and its own
 * tests are the ones that hold the format. What this file is for is getting one
 * onto disk.
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { crc32 } from "node:zlib";

/**
 * A `description.xml` for a fixture with one mode, one beam and a body.
 *
 * Written out rather than built from parts: the thing under test is a reading
 * of a document, and a document assembled by this file would be a reading of
 * this file's idea of one.
 */
export function description(manufacturer: string, name: string): string {
  const identity = "{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}";
  return `<?xml version="1.0" encoding="UTF-8"?>
<GDTF DataVersion="1.2">
  <FixtureType Name="${name}" ShortName="T1" Manufacturer="${manufacturer}"
               FixtureTypeID="9F4A0000-0000-4000-8000-00000000E2E1">
    <AttributeDefinitions>
      <Attributes>
        <Attribute Name="Dimmer" Pretty="Dim"/>
        <Attribute Name="Gobo1" Pretty="G1"/>
      </Attributes>
    </AttributeDefinitions>
    <Wheels>
      <Wheel Name="Gobo1">
        <Slot Name="Open"/>
        <Slot Name="Triangles" MediaFileName="gobo_triangles"/>
      </Wheel>
    </Wheels>
    <Models>
      <Model Name="Body" File="body" Length="0.34" Width="0.34" Height="0.55"/>
    </Models>
    <Geometries>
      <Geometry Name="Body" Model="Body" Position="${identity}">
        <Beam Name="Beam" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,400,1}"
              BeamAngle="13" LuminousFlux="11000" ColorTemperature="6500"/>
      </Geometry>
    </Geometries>
    <DMXModes>
      <DMXMode Name="Mode 1" Geometry="Body">
        <DMXChannels>
          <DMXChannel Offset="1,2">
            <LogicalChannel Attribute="Pan">
              <ChannelFunction Attribute="Pan" PhysicalFrom="-270" PhysicalTo="270"/>
            </LogicalChannel>
          </DMXChannel>
          <DMXChannel Offset="3">
            <LogicalChannel Attribute="Dimmer">
              <ChannelFunction Attribute="Dimmer"/>
            </LogicalChannel>
          </DMXChannel>
          <DMXChannel Offset="4">
            <LogicalChannel Attribute="Gobo1">
              <ChannelFunction Attribute="Gobo1" Wheel="Gobo1">
                <ChannelSet DMXFrom="0/1" WheelSlotIndex="1"/>
                <ChannelSet DMXFrom="10/1" WheelSlotIndex="2"/>
              </ChannelFunction>
            </LogicalChannel>
          </DMXChannel>
        </DMXChannels>
      </DMXMode>
    </DMXModes>
  </FixtureType>
</GDTF>`;
}

/** Writes a one-entry ZIP archive holding `description.xml` at `path`. */
export function writeGdtf(path: string, xml: string): void {
  const name = Buffer.from("description.xml", "utf8");
  const body = Buffer.from(xml, "utf8");
  const sum = crc32(body);

  const local = Buffer.alloc(30);
  local.writeUInt32LE(0x04034b50, 0); // local file header
  local.writeUInt16LE(20, 4); // version needed
  local.writeUInt16LE(0, 6); // flags
  local.writeUInt16LE(0, 8); // stored
  local.writeUInt32LE(0, 10); // time and date
  local.writeUInt32LE(sum, 14);
  local.writeUInt32LE(body.length, 18);
  local.writeUInt32LE(body.length, 22);
  local.writeUInt16LE(name.length, 26);
  local.writeUInt16LE(0, 28); // extra

  const central = Buffer.alloc(46);
  central.writeUInt32LE(0x02014b50, 0); // central directory header
  central.writeUInt16LE(20, 4); // made by
  central.writeUInt16LE(20, 6); // version needed
  central.writeUInt16LE(0, 8); // flags
  central.writeUInt16LE(0, 10); // stored
  central.writeUInt32LE(0, 12); // time and date
  central.writeUInt32LE(sum, 16);
  central.writeUInt32LE(body.length, 20);
  central.writeUInt32LE(body.length, 24);
  central.writeUInt16LE(name.length, 28);
  central.writeUInt16LE(0, 30); // extra
  central.writeUInt16LE(0, 32); // comment
  central.writeUInt16LE(0, 34); // disk
  central.writeUInt16LE(0, 36); // internal attributes
  central.writeUInt32LE(0, 38); // external attributes
  central.writeUInt32LE(0, 42); // where the local header is

  const directoryAt = local.length + name.length + body.length;
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0); // end of central directory
  end.writeUInt16LE(0, 4); // this disk
  end.writeUInt16LE(0, 6); // the directory's disk
  end.writeUInt16LE(1, 8); // entries here
  end.writeUInt16LE(1, 10); // entries in all
  end.writeUInt32LE(central.length + name.length, 12);
  end.writeUInt32LE(directoryAt, 16);
  end.writeUInt16LE(0, 20); // comment

  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, Buffer.concat([local, name, body, central, name, end]));
}
