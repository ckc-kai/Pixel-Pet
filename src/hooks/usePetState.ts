// Pet-state subscription hook.
//
// Subscribes to the Rust FSM via the `petSubscribeState` IPC channel. The
// backend seeds the channel with the current state on connect, so consumers
// see a valid `PetState` on the very first render after mount.
//
// Teardown: the backend keeps the forwarder task alive while the channel
// remains referenced from JS. Once the hook unmounts we drop our reference
// (`channelRef.current = null`); the next message attempt fails the task's
// `channel.send` and the task exits naturally. We additionally null the
// onmessage handler so any in-flight message is silently dropped during
// unmount.
//
// MAX_SUBSCRIBERS on the backend is 4; in production we only mount one
// subscription per window, so this hook never approaches the cap.

import { useEffect, useState } from "react";
import { Channel } from "@tauri-apps/api/core";
import { commands, type PetState } from "../lib/types/bindings";

/**
 * Subscribe to live pet-state updates.
 *
 * Returns `null` until the first message arrives from the backend (typically
 * within a tick of mount, since the channel is seeded on subscribe).
 */
export function usePetState(): PetState | null {
  const [state, setState] = useState<PetState | null>(null);

  useEffect(() => {
    let active = true;
    const channel = new Channel<PetState>();

    channel.onmessage = (next) => {
      if (!active) return;
      setState(next);
    };

    let unsubscribed = false;
    commands.petSubscribeState(channel).then((result) => {
      if (unsubscribed) return;
      if (result.status === "error") {
        // BadRequest (subscriber cap) or transport failure — log only.
        // The hook will simply never produce a state; the renderer should
        // surface a fallback (e.g. base "working" visual).
        // eslint-disable-next-line no-console
        console.warn("pet_subscribe_state failed", result.error);
      }
    });

    return () => {
      active = false;
      unsubscribed = true;
      // Drop the message handler so any straggler message during teardown
      // is ignored rather than triggering a setState on an unmounted tree.
      channel.onmessage = () => {};
    };
  }, []);

  return state;
}
