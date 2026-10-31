"""window.py: drive Minecraft's window on the virtual display, as a player.

  window.py key <keysym>...   press keys (e.g. F2, Return, Tab)
  window.py click <x> <y>     left-click at window coordinates
  window.py close             close the window, as its close button would

Xvfb runs no window manager, so close sends WM_DELETE_WINDOW itself.
"""

import sys
import time

from Xlib import X, XK, display
from Xlib.ext import xtest
from Xlib.protocol import event

d = display.Display()
root = d.screen().root


def window():
    for _ in range(20):
        for w in root.query_tree().children:
            name = w.get_full_property(d.intern_atom("_NET_WM_NAME"), X.AnyPropertyType)
            title = name.value.decode() if name else w.get_wm_name() or ""
            if title.startswith("Minecraft") and w.get_attributes().map_state == X.IsViewable:
                return w
        time.sleep(1)
    sys.exit("no Minecraft window")


def tap(kind, code):
    xtest.fake_input(d, kind, code)
    d.sync()
    time.sleep(0.1)
    xtest.fake_input(d, kind + 1, code)
    d.sync()
    time.sleep(0.5)


w = window()
w.set_input_focus(X.RevertToParent, X.CurrentTime)
cmd, args = sys.argv[1], sys.argv[2:]
if cmd == "key":
    for k in args:
        tap(X.KeyPress, d.keysym_to_keycode(XK.string_to_keysym(k)))
elif cmd == "click":
    w.warp_pointer(int(args[0]), int(args[1]))
    d.sync()
    tap(X.ButtonPress, 1)
elif cmd == "close":
    print(f"closing {w.get_wm_name()!r}")
    protocols = d.intern_atom("WM_PROTOCOLS")
    delete = d.intern_atom("WM_DELETE_WINDOW")
    w.send_event(
        event.ClientMessage(
            window=w, client_type=protocols, data=(32, [delete, X.CurrentTime, 0, 0, 0])
        )
    )
d.flush()
