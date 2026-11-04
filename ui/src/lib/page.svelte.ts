// Lets a page put controls into the shell's title row, next to the h1:
//
//   {#snippet tools()}<Segmented ... />{/snippet}
//   $effect(() => usePageToolbar(tools));

import type { Snippet } from "svelte";

let toolbar = $state<Snippet | null>(null);

export const pageToolbar = {
  get current(): Snippet | null {
    return toolbar;
  },
};

/** Call inside `$effect`; the toolbar is removed when the effect is torn down. */
export function usePageToolbar(s: Snippet): () => void {
  toolbar = s;
  return () => {
    if (toolbar === s) toolbar = null;
  };
}
