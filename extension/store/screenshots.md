# Screenshots to take

Both stores take 1280x800 PNG or JPEG (Chrome also 640x400; AMO any size
up to 2400x1800 with the same 1.6 ratio looks best). Chrome also wants a
440x280 small promo tile. Take them on a light desktop theme, with demo
data (`tools/seed_demo.py <data dir>\activity.db 14` while the app is
stopped) so no real browsing shows, and the browser zoom at 100 %.

The popup itself is 340 px wide; place it at its real size on a plain
background or over a blurred browser window, not scaled up.

1. **Popup, paired** (1280x800): status "Paired as chrome on port 47821",
   Now "Counting" on a recognisable site (github.com), Today with five
   hosts and a total over an hour, one rule part way ("Rest break in 12m").
   Caption: "See what is counted and today's time per site."
2. **Popup with a nag** (1280x800): a rule due or nagging with its message,
   the Snooze and Skip buttons visible. Make one due quickly with a short
   rule in the app's rules, for example
   `trigger = { kind = "interval", every = "1m", break_for = "30s" }`.
   Caption: "Snooze, skip or start a break from the toolbar."
3. **Options page with the pairing code** (1280x800): the app's pairing
   window or dashboard Settings showing a code next to the extension's
   options page with the same code typed in.
   Caption: "Pair once with a six-digit code. Nothing leaves your computer."
4. **The dashboard's Web page** (1280x800): the app's dashboard showing web
   time per site for a week of demo data.
   Caption: "Your web time next to every other app, in the Nagsworth
   dashboard."
5. **Small promo tile** (440x280, Chrome only): the Nagsworth icon on the
   accent color (#4f46e5) with the word "Nagsworth" and "Breaks that count
   your browser too" in white.

The end-to-end harness saves a raw popup capture to
`%TEMP%\an-ext-e2e\popup-edge.png` (`tools\ext_e2e.ps1 -Browser edge`); it
is a quick reference for layout, not a store image.
