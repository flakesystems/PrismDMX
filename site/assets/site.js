/*
 * prismdmx.de — die Startseite, bewegt.
 *
 * Der Unterschied zu `web/` in einem Satz: **dort darf kein Skript sein, hier
 * schon.** Eine Handbuchseite, die ein Skript braucht, um lesbar zu sein, ist
 * eine Seite, die man auf dem Rechner im Rack nicht lesen kann — deshalb hat
 * `prism-web` einen Test, der jedes `<script>` zurückweist. Eine Startseite hat
 * die umgekehrte Aufgabe: sie soll zeigen, wie sich die Software anfühlt, und
 * das geht ohne Bewegung nicht.
 *
 * Die Grenze, die dafür trotzdem gilt:
 *
 *   - **Ohne Skript bleibt die Seite vollständig lesbar.** Jeder Satz, jeder
 *     Link und jede Zahl steht im HTML. Was das Skript hinzufügt, ist Bewegung
 *     und vier Nachbauten — nie Inhalt, den man sonst nicht bekäme.
 *   - **Keine Abhängigkeit.** Kein Framework, kein Bundler, keine dritte Partei,
 *     die mitliest. Eine Datei, die der Browser so nimmt, wie sie im Repository
 *     steht.
 *   - **`prefers-reduced-motion` hält alles an.** Nicht gedrosselt: die
 *     Zeitgeber laufen gar nicht erst, und die Demos zeigen sich in ihrem
 *     Endzustand statt zu spielen.
 *
 * Die Demos sind Nachbauten, keine Emulation. Sie sagen das auch — die echte
 * Oberfläche ist dichter und hat vierzehn Fenstertypen.
 */

(function () {
  "use strict";

  var STILL = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  var $ = function (sel, root) { return (root || document).querySelector(sel); };
  var $$ = function (sel, root) { return Array.prototype.slice.call((root || document).querySelectorAll(sel)); };
  var clamp = function (n, lo, hi) { return n < lo ? lo : n > hi ? hi : n; };

  /* ====================================================================== */
  /* Kopfleiste                                                             */
  /* ====================================================================== */

  var top = $(".top");
  var onScroll = function () { top.classList.toggle("stuck", window.scrollY > 12); };
  onScroll();
  window.addEventListener("scroll", onScroll, { passive: true });

  var burger = $(".burger");
  var mobile = $(".nav-mobil");
  burger.addEventListener("click", function () {
    var open = burger.getAttribute("aria-expanded") === "true";
    burger.setAttribute("aria-expanded", String(!open));
    mobile.hidden = open;
  });
  $$(".nav-mobil a").forEach(function (a) {
    a.addEventListener("click", function () {
      burger.setAttribute("aria-expanded", "false");
      mobile.hidden = true;
    });
  });

  /* Welcher Abschnitt gerade dran ist. */
  var sections = $$("main section[id]");
  var navLinks = {};
  $$(".nav a[href^='#']").forEach(function (a) { navLinks[a.getAttribute("href").slice(1)] = a; });
  if ("IntersectionObserver" in window) {
    var spy = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        var link = navLinks[e.target.id];
        if (link && e.isIntersecting) {
          Object.keys(navLinks).forEach(function (k) { navLinks[k].classList.remove("here"); });
          link.classList.add("here");
        }
      });
    }, { rootMargin: "-45% 0px -50% 0px" });
    sections.forEach(function (s) { spy.observe(s); });
  }

  /* ====================================================================== */
  /* Einblenden und Zählen                                                  */
  /* ====================================================================== */

  var revealables = $$(".reveal");
  if (STILL || !("IntersectionObserver" in window)) {
    revealables.forEach(function (el) { el.classList.add("in"); });
  } else {
    var eye = new IntersectionObserver(function (entries) {
      entries.forEach(function (e, i) {
        if (!e.isIntersecting) return;
        var el = e.target;
        setTimeout(function () { el.classList.add("in"); }, Math.min(i, 5) * 65);
        eye.unobserve(el);
      });
    }, { rootMargin: "0px 0px -12% 0px", threshold: 0.08 });
    revealables.forEach(function (el) { eye.observe(el); });
  }

  /* Die vier Zahlen unter dem Aufmacher zählen sich einmal hoch. */
  var counters = $$(".stats b[data-count]");
  var countUp = function (el) {
    var target = parseInt(el.getAttribute("data-count"), 10);
    var suffix = el.getAttribute("data-suffix") || "";
    var group = el.hasAttribute("data-sep");
    var started = performance.now();
    var dur = 1100;
    var step = function (now) {
      var t = clamp((now - started) / dur, 0, 1);
      var eased = 1 - Math.pow(1 - t, 3);
      var v = Math.round(target * eased);
      el.textContent = (group ? v.toLocaleString("de-DE").replace(/\./g, " ") : String(v)) + suffix;
      if (t < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  };
  if (!STILL && "IntersectionObserver" in window) {
    var numEye = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (!e.isIntersecting) return;
        countUp(e.target);
        numEye.unobserve(e.target);
      });
    }, { threshold: 0.6 });
    counters.forEach(function (el) { numEye.observe(el); });
  }

  /* Der Lichtfleck unter dem Zeiger auf den Karten. */
  if (!STILL && window.matchMedia("(pointer: fine)").matches) {
    $$(".card").forEach(function (card) {
      card.addEventListener("pointermove", function (ev) {
        var r = card.getBoundingClientRect();
        card.style.setProperty("--mx", (ev.clientX - r.left) + "px");
        card.style.setProperty("--my", (ev.clientY - r.top) + "px");
      });
    });
  }

  /* ====================================================================== */
  /* Der Beweis: das Fenster darf sterben                                   */
  /* ====================================================================== */

  (function proof() {
    var box = $("#proof");
    if (!box) return;

    var barBox = $("#proof-bars");
    var BARS = 28;
    var bars = [];
    for (var i = 0; i < BARS; i++) {
      var b = document.createElement("i");
      barBox.appendChild(b);
      bars.push(b);
    }

    var countOut = $("#proof-count");
    var gapOut = $("#proof-gap");
    var note = $("#proof-note");
    var client = $("#proc-client");
    var clientState = $("#client-state");
    var clientLed = $("#client-led");
    var kill = $("#kill");

    var frames = 0;
    var phase = 0;
    var dead = false;

    var draw = function () {
      phase += 0.16;
      for (var i = 0; i < BARS; i++) {
        var h = 12 + 44 * (0.5 + 0.5 * Math.sin(phase - i * 0.42)) + 14 * (0.5 + 0.5 * Math.sin(phase * 0.31 + i));
        bars[i].style.height = Math.round(clamp(h, 3, 74)) + "px";
      }
      /* 44 Hz, aber gezählt wird in Zehnerschritten — die Zahl soll lesbar
         bleiben und nicht flimmern. */
      frames += 4;
      countOut.textContent = frames.toLocaleString("de-DE");
    };

    if (STILL) {
      for (var j = 0; j < BARS; j++) bars[j].style.height = (10 + (j * 37) % 60) + "px";
      countOut.textContent = "44 pro Sekunde";
      gapOut.textContent = "0";
    } else {
      setInterval(draw, 90);
    }

    var revive;
    kill.addEventListener("click", function () {
      clearTimeout(revive);
      if (!dead) {
        dead = true;
        box.classList.add("dead");
        client.classList.add("gone");
        clientLed.classList.remove("ok");
        clientLed.classList.add("bad");
        clientState.textContent = "abgestürzt";
        note.textContent = "Die Oberfläche ist weg. Der Ausgang zählt weiter, die Show läuft, und der X-Touch bedient das Pult, als wäre nichts gewesen.";
        kill.textContent = "Das Fenster neu starten";
        gapOut.textContent = "0";
        revive = setTimeout(function () { if (dead) kill.click(); }, 6500);
      } else {
        dead = false;
        box.classList.remove("dead");
        client.classList.remove("gone");
        clientLed.classList.add("ok");
        clientLed.classList.remove("bad");
        clientState.textContent = "verbunden";
        note.textContent = "Zurück — und es war kein Wiederherstellen, sondern ein gewöhnliches Verbinden. Der Client findet den Zustand vor, den die Engine inzwischen hält.";
        kill.textContent = "Das Fenster abstürzen lassen";
      }
    });
  })();

  /* ====================================================================== */
  /* Die Demos                                                              */
  /* ====================================================================== */

  var typed = $("#typed");
  var hint = $("#hint");

  /* --- Demo 1: die Kommandozeile ---------------------------------------- */

  var Line = (function () {
    var rig = $("#rig");
    var hintOut = $("#line-hint");
    var COUNT = 6;
    var lamps = [];
    var level = [0, 0, 0, 0, 0, 0];

    for (var i = 0; i < COUNT; i++) {
      var el = document.createElement("div");
      el.className = "lamp";
      el.innerHTML = '<div class="bulb"></div><span class="no">Fixture ' + (i + 1) + '</span><span class="lvl">0 %</span>';
      rig.appendChild(el);
      lamps.push(el);
    }

    var paint = function () {
      for (var i = 0; i < COUNT; i++) {
        var v = level[i];
        var bulb = lamps[i].firstChild;
        lamps[i].classList.toggle("lit", v > 0);
        if (v <= 0) {
          bulb.style.background = "#0d0f13";
          bulb.style.boxShadow = "inset 0 0 0 1px #ffffff08";
        } else {
          var a = v / 100;
          bulb.style.background = "rgb(" + Math.round(60 + 195 * a) + "," + Math.round(40 + 190 * a) + "," + Math.round(20 + 170 * a) + ")";
          bulb.style.boxShadow = "0 0 " + Math.round(4 + 22 * a) + "px rgba(255,205,130," + (0.28 * a).toFixed(2) + "), inset 0 0 0 1px #ffffff14";
        }
        lamps[i].querySelector(".lvl").textContent = v + " %";
      }
    };

    var set = function (from, to, v) {
      for (var i = from - 1; i < to; i++) level[i] = v;
      for (var k = 0; k < COUNT; k++) lamps[k].classList.toggle("sel", k >= from - 1 && k < to);
      paint();
    };

    var script = [
      { line: "1 thru 6 at full", run: function () { set(1, 6, 100); }, say: "Sechs Fixtures, eine Zeile. „full\" ist 100 %." },
      { line: "4 thru 6 at 40", run: function () { set(4, 6, 40); }, say: "Ein Bereich daraus, auf 40 %. Die Auswahl ist die Kante." },
      { line: "2 at 0", run: function () { set(2, 2, 0); }, say: "Einzeln geht auch. Alles, was hier steht, ist auf eine X-Touch-Taste legbar." },
      { line: "group 1 at 75", run: function () { set(1, 3, 75); }, say: "Gruppen sind benannte Auswahlen — hier die ersten drei." },
      { line: "store cue 1", run: function () { for (var k = 0; k < COUNT; k++) lamps[k].classList.remove("sel"); }, say: "Und weg damit in Cue 1. Der Programmer hält, bis du speicherst oder löschst." },
      { line: "clear", run: function () { set(1, 6, 0); for (var k = 0; k < COUNT; k++) lamps[k].classList.remove("sel"); }, say: "Clear räumt den Programmer. Von vorn." }
    ];

    paint();

    return {
      script: script,
      hint: hintOut,
      reset: function () {
        level = [0, 0, 0, 0, 0, 0];
        paint();
      },
      settle: function () {
        level = [75, 75, 75, 40, 40, 40];
        paint();
        hintOut.textContent = "Endzustand von „group 1 at 75\" über „1 thru 6 at full\".";
      }
    };
  })();

  /* --- Demo 2: Executoren und Cues -------------------------------------- */

  var Exec = (function () {
    var bar = $("#execbar");
    var rows = $("#cue-rows");

    var cues = [
      { n: "1", name: "Vorstellung · Saallicht", fade: "3", delay: "—" },
      { n: "2", name: "Blackout", fade: "2", delay: "—" },
      { n: "3", name: "Auftritt links", fade: "1,5", delay: "0,5" },
      { n: "4", name: "Solo · warm", fade: "4", delay: "—" },
      { n: "5", name: "Umbau", fade: "0", delay: "—" },
      { n: "6", name: "Schluss · Applaus", fade: "6", delay: "—" }
    ];

    cues.forEach(function (c) {
      var tr = document.createElement("tr");
      tr.innerHTML = "<td>" + c.n + "</td><td>" + c.name + "</td><td>" + c.fade + " s</td><td>" + c.delay + "</td>";
      rows.appendChild(tr);
    });

    var names = ["Saal", "Bühne warm", "Gegenlicht", "Effekt"];
    var strips = [];
    names.forEach(function (name) {
      var el = document.createElement("div");
      el.className = "strip";
      el.innerHTML =
        '<span class="lbl">' + name + '</span>' +
        '<div class="fader"><div class="fill" style="height:0"></div><div class="knob" style="bottom:0"></div></div>' +
        '<span class="val">0</span>' +
        '<span class="go">Go</span>';
      bar.appendChild(el);
      strips.push(el);
    });

    var setFader = function (i, v) {
      var s = strips[i];
      s.querySelector(".fill").style.height = v + "%";
      s.querySelector(".knob").style.bottom = "calc(" + v + "% - 2px)";
      s.querySelector(".val").textContent = v;
      s.classList.toggle("live", v > 0);
    };

    var setCue = function (i) {
      $$("tr", rows).forEach(function (tr, k) { tr.classList.toggle("now", k === i); });
    };

    var flashGo = function (i) {
      var go = strips[i].querySelector(".go");
      go.classList.add("hit");
      setTimeout(function () { go.classList.remove("hit"); }, 420);
    };

    var script = [
      { line: "executor 1 at full", run: function () { setFader(0, 100); setCue(0); } },
      { line: "go executor 1", run: function () { flashGo(0); setCue(1); } },
      { line: "executor 2 at 65", run: function () { setFader(1, 65); } },
      { line: "go executor 1", run: function () { flashGo(0); setCue(2); } },
      { line: "executor 3 at 80", run: function () { setFader(2, 80); } },
      { line: "go executor 1", run: function () { flashGo(0); setCue(3); } },
      { line: "executor 4 at 45", run: function () { setFader(3, 45); } },
      { line: "goback executor 1", run: function () { flashGo(0); setCue(2); } },
      { line: "off executor 1 thru 4", run: function () { [0, 1, 2, 3].forEach(function (i) { setFader(i, 0); }); setCue(0); } }
    ];

    setCue(0);

    return {
      script: script,
      hint: null,
      reset: function () { [0, 1, 2, 3].forEach(function (i) { setFader(i, 0); }); setCue(0); },
      settle: function () { setFader(0, 100); setFader(1, 65); setFader(2, 80); setCue(2); }
    };
  })();

  /* --- Demo 3: der Programmer ------------------------------------------- */

  var Prog = (function () {
    var box = $("#encoders");
    var swatch = $("#swatch");
    var hex = $("#swatch-hex");
    var touchedOut = $("#prog-touched");

    var params = [
      { name: "Rot", color: "#f87171" },
      { name: "Grün", color: "#4ade80" },
      { name: "Blau", color: "#60a5fa" },
      { name: "Weiß", color: "#e8e9ec" }
    ];
    var value = [0, 0, 0, 0];
    var encs = [];
    var R = 26;
    var C = 2 * Math.PI * R * 0.75; /* 270° Skala */

    params.forEach(function (p, i) {
      var el = document.createElement("div");
      el.className = "enc";
      el.innerHTML =
        '<div class="dial">' +
        '<svg viewBox="0 0 62 62">' +
        '<circle class="track" cx="31" cy="31" r="' + R + '" stroke-dasharray="' + C + ' 999"></circle>' +
        '<circle class="arc" cx="31" cy="31" r="' + R + '" stroke="' + p.color + '" stroke-dasharray="' + C + ' 999" stroke-dashoffset="' + C + '"></circle>' +
        "</svg>" +
        '<span class="num">0</span></div>' +
        '<span class="name">' + p.name + "</span>";
      box.appendChild(el);
      encs.push(el);
    });

    var paint = function () {
      var touched = 0;
      for (var i = 0; i < 4; i++) {
        var v = value[i];
        if (v > 0) touched++;
        encs[i].querySelector(".arc").style.strokeDashoffset = String(C * (1 - v / 100));
        encs[i].querySelector(".num").textContent = String(v);
        encs[i].classList.toggle("touched", v > 0);
      }
      var mix = function (i) { return Math.round(255 * (value[i] / 100)); };
      var w = Math.round(255 * (value[3] / 100) * 0.75);
      var r = clamp(mix(0) + w, 0, 255), g = clamp(mix(1) + w, 0, 255), b = clamp(mix(2) + w, 0, 255);
      swatch.style.background = "rgb(" + r + "," + g + "," + b + ")";
      hex.textContent = "#" + [r, g, b].map(function (n) { return ("0" + n.toString(16)).slice(-2); }).join("").toUpperCase();
      touchedOut.textContent = String(touched);
    };

    /* Ein Encoder fährt nicht, er wird gedreht — also über ein paar Frames. */
    var ease = function (i, to, done) {
      if (STILL) { value[i] = to; paint(); if (done) done(); return; }
      var from = value[i];
      var t0 = performance.now();
      var step = function (now) {
        var t = clamp((now - t0) / 620, 0, 1);
        value[i] = Math.round(from + (to - from) * (1 - Math.pow(1 - t, 3)));
        paint();
        if (t < 1) requestAnimationFrame(step); else if (done) done();
      };
      requestAnimationFrame(step);
    };

    var script = [
      { line: "1 thru 6 at full", run: function () { } },
      { line: "red at 100", run: function () { ease(0, 100); } },
      { line: "blue at 60", run: function () { ease(2, 60); } },
      { line: "green at 25", run: function () { ease(1, 25); } },
      { line: "white at 35", run: function () { ease(3, 35); } },
      { line: "store preset color 4 \"Magenta warm\"", run: function () { } },
      { line: "clear", run: function () { ease(0, 0); ease(1, 0); ease(2, 0); ease(3, 0); } }
    ];

    paint();

    return {
      script: script,
      hint: null,
      reset: function () { value = [0, 0, 0, 0]; paint(); },
      settle: function () { value = [100, 25, 60, 35]; paint(); }
    };
  })();

  /* --- Demo 4: das DMX-Sheet -------------------------------------------- */

  var Sheet = (function () {
    var grid = $("#sheet");
    var framesOut = $("#tele-frames");
    var hzOut = $("#tele-hz");
    var paintOut = $("#tele-paint");
    var CELLS = 256;
    var cells = [];

    for (var i = 0; i < CELLS; i++) {
      var c = document.createElement("div");
      c.className = "cell";
      grid.appendChild(c);
      cells.push(c);
    }

    var t = 0;
    var frames = 0;

    var render = function () {
      t += 0.09;
      for (var i = 0; i < CELLS; i++) {
        var x = i % 32, y = (i / 32) | 0;
        var v = 0.5 + 0.5 * Math.sin(t - x * 0.22 + y * 0.5);
        v = Math.pow(v, 2.4);
        cells[i].style.background = v < 0.03
          ? "#0d0f13"
          : "rgba(" + Math.round(120 + 135 * v) + "," + Math.round(70 + 90 * v) + ",252," + (0.15 + 0.85 * v).toFixed(2) + ")";
      }
      frames += 1;
      framesOut.textContent = frames.toLocaleString("de-DE");
      hzOut.textContent = (30.0 + (frames % 7) * 0.05).toFixed(1).replace(".", ",") + " Hz";
      paintOut.textContent = (0.18 + (frames % 5) * 0.01).toFixed(2).replace(".", ",") + " ms";
    };

    var timer = null;

    return {
      script: null,
      hint: null,
      start: function () { if (!STILL && !timer) timer = setInterval(render, 60); },
      stop: function () { clearInterval(timer); timer = null; },
      reset: function () { },
      settle: function () { render(); }
    };
  })();

  /* --- Der Spielleiter --------------------------------------------------- */

  var demos = { line: Line, exec: Exec, prog: Prog, dmx: Sheet };
  var current = "line";
  var playing = !STILL;

  var timer = null;
  var step = 0;
  var chars = 0;
  var stage = "typing"; /* typing → firing → resting */
  var rest = 0;

  var stop = function () {
    clearInterval(timer);
    timer = null;
    if (Sheet.stop) Sheet.stop();
  };

  var tick = function () {
    var demo = demos[current];
    if (!demo || !demo.script) return;
    var item = demo.script[step % demo.script.length];

    if (stage === "typing") {
      chars++;
      typed.textContent = item.line.slice(0, chars);
      if (chars >= item.line.length) { stage = "firing"; rest = 0; }
      return;
    }

    if (stage === "firing") {
      rest++;
      hint.classList.add("fire");
      if (rest >= 5) {
        item.run();
        if (demo.hint && item.say) demo.hint.textContent = item.say;
        stage = "resting";
        rest = 0;
      }
      return;
    }

    rest++;
    hint.classList.remove("fire");
    if (rest >= 22) {
      step++;
      chars = 0;
      stage = "typing";
      typed.textContent = "";
    }
  };

  var start = function () {
    stop();
    if (!playing || STILL) return;
    if (current === "dmx") { Sheet.start(); typed.textContent = "dmx sheet 1"; return; }
    timer = setInterval(tick, 55);
  };

  var show = function (name) {
    stop();
    current = name;
    step = 0; chars = 0; rest = 0; stage = "typing";
    typed.textContent = "";
    hint.classList.remove("fire");

    $$(".demo-tabs button[role=tab]").forEach(function (b) {
      var on = b.id === "tab-" + name;
      b.classList.toggle("on", on);
      b.setAttribute("aria-selected", String(on));
    });
    $$(".pane").forEach(function (p) {
      var on = p.id === "pane-" + name;
      p.classList.toggle("on", on);
      p.hidden = !on;
    });

    Object.keys(demos).forEach(function (k) { if (demos[k].reset) demos[k].reset(); });

    if (STILL) {
      Object.keys(demos).forEach(function (k) { if (demos[k].settle) demos[k].settle(); });
      typed.textContent = demos[name].script ? demos[name].script[1].line : "dmx sheet 1";
      return;
    }
    start();
  };

  $$(".demo-tabs button[role=tab]").forEach(function (b) {
    b.addEventListener("click", function () { show(b.id.slice(4)); });
  });

  var playBtn = $("#demo-play");
  playBtn.addEventListener("click", function () {
    playing = !playing;
    playBtn.setAttribute("data-playing", playing ? "1" : "0");
    playBtn.setAttribute("aria-label", playing ? "Demo anhalten" : "Demo abspielen");
    if (playing) start(); else stop();
  });

  /* Nur laufen lassen, solange es jemand sehen kann — ein Zeitgeber in einem
     Reiter, den niemand ansieht, ist verbrannter Akku. */
  if ("IntersectionObserver" in window) {
    new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting && playing) start(); else stop();
      });
    }, { threshold: 0.12 }).observe($("#demo"));
  }
  document.addEventListener("visibilitychange", function () {
    if (document.hidden) stop(); else if (playing) start();
  });

  show("line");
})();
