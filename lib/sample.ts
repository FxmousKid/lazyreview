import type { ChangedFile, PullRequest } from "./review";
function file(filename: string, before: string, after: string): ChangedFile {
  const old = before ? before.split("\n") : [],
    next = after.split("\n");
  const lengths = Array.from({ length: old.length + 1 }, () =>
    new Array(next.length + 1).fill(0),
  );
  for (let i = old.length - 1; i >= 0; i--)
    for (let j = next.length - 1; j >= 0; j--)
      lengths[i][j] =
        old[i] === next[j]
          ? lengths[i + 1][j + 1] + 1
          : Math.max(lengths[i + 1][j], lengths[i][j + 1]);
  const lines: string[] = [];
  let i = 0,
    j = 0;
  while (i < old.length || j < next.length) {
    if (i < old.length && j < next.length && old[i] === next[j]) {
      lines.push(" " + old[i++]);
      j++;
    } else if (
      i < old.length &&
      (j === next.length || lengths[i + 1][j] >= lengths[i][j + 1])
    )
      lines.push("-" + old[i++]);
    else lines.push("+" + next[j++]);
  }
  return {
    filename,
    sha: "sample-file-" + filename,
    status: before ? "modified" : "added",
    additions: lines.filter((l) => l.startsWith("+")).length,
    deletions: lines.filter((l) => l.startsWith("-")).length,
    patch: `@@ -${old.length ? 1 : 0},${old.length} +1,${next.length} @@\n${lines.join("\n")}`,
    content: after,
    originalContent: before,
  };
}
const before = `import { useState } from 'react';
import { Dialog } from '@/components/ui/dialog';
import { Search } from 'lucide-react';

import { commands } from './commands';
import type { CommandMenuProps } from './types';

export function CommandMenu({ open, onOpenChange }: CommandMenuProps) {
  const [query, setQuery] = useState('');
  const results = commands.filter((command) =>
    command.label.toLowerCase().includes(query.toLowerCase())
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <input value={query} onChange={(e) => setQuery(e.target.value)} />
      {results.map((command) => (
        <button key={command.id} onClick={command.run}>
          {command.label}
        </button>
      ))}
    </Dialog>
  );
}`;
const after = `import { useEffect, useState } from 'react';
import { Dialog } from '@/components/ui/dialog';
import { Search } from 'lucide-react';

import { commands } from './commands';
import { useKeyboardNavigation } from './use-keyboard-navigation';
import { filterCommands } from './filter-commands';
import type { CommandMenuProps } from './types';

export function CommandMenu({ open, onOpenChange }: CommandMenuProps) {
  const [query, setQuery] = useState('');
  const results = filterCommands(commands, query);

  const { activeIndex, onKeyDown, reset } = useKeyboardNavigation({
    count: results.length,
    onSelect: (index) => {
      results[index]?.run();
      onOpenChange(false);
    },
  });

  useEffect(() => {
    reset();
  }, [query, reset]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <div className="command-search">
        <Search size={18} aria-hidden="true" />
        <input
          role="combobox"
          aria-expanded={open}
          aria-controls="command-results"
          aria-activedescendant={results[activeIndex]?.id}
          placeholder="Search commands…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
        />
      </div>
      <ul id="command-results" role="listbox">
        {results.map((command, index) => (
          <li key={command.id} id={command.id} role="option"
            aria-selected={index === activeIndex}>
            <button onClick={() => { command.run(); onOpenChange(false); }}>
              <command.icon size={16} />
              <span>{command.label}</span>
              <kbd>{command.shortcut}</kbd>
            </button>
          </li>
        ))}
      </ul>
    </Dialog>
  );
}`;
const hook = `import { useCallback, useState } from 'react';
import type { KeyboardEvent } from 'react';

type Options = { count: number; onSelect: (index: number) => void };

export function useKeyboardNavigation({ count, onSelect }: Options) {
  const [activeIndex, setActiveIndex] = useState(0);
  const reset = useCallback(() => setActiveIndex(0), []);

  function onKeyDown(event: KeyboardEvent) {
    if (count === 0) return;
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setActiveIndex((index) => (index + 1) % count);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setActiveIndex((index) => (index - 1 + count) % count);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      onSelect(activeIndex);
    }
  }

  return { activeIndex, onKeyDown, reset };
}`;
export const sample: PullRequest = {
  createdAt: "2026-10-12T08:00:00Z",
  updatedAt: "2026-10-13T09:00:00Z",
  owner: "acme",
  repo: "atlas",
  number: 128,
  id: "sample-pr",
  title: "Add command menu and keyboard navigation",
  body: "Adds a searchable command menu with keyboard navigation.\n\nThe menu groups common actions, supports arrow keys and Enter, and keeps the active result visible to assistive technology.\n\nReview focus\n• Keyboard navigation and empty results\n• Focus behavior when opening and closing\n• Coverage of filtering and selection",
  author: "sarahchen",
  state: "open",
  head: "a4f81d7c29b610dca74e463022efc08ea0285d61",
  base: "e793aa1c29b610dca74e463022efc08ea0285d61",
  headRef: "feat/command-menu",
  baseRef: "main",
  url: "",
  demo: true,
  viewer: "you",
  warnings: [],
  files: [
    file("src/components/command-menu/command-menu.tsx", before, after),
    file("src/components/command-menu/use-keyboard-navigation.ts", "", hook),
    file(
      "src/components/command-menu/filter-commands.ts",
      "",
      `import type { Command } from './types';\n\nexport function filterCommands(commands: Command[], query: string) {\n  const normalized = query.trim().toLowerCase();\n  if (!normalized) return commands;\n  return commands.filter(({ label, keywords = [] }) =>\n    [label, ...keywords].some((value) =>\n      value.toLowerCase().includes(normalized)\n    )\n  );\n}`,
    ),
    file(
      "src/components/command-menu/types.ts",
      "export type Command = { id: string; label: string; run: () => void };",
      `import type { LucideIcon } from 'lucide-react';\n\nexport type Command = {\n  id: string;\n  label: string;\n  keywords?: string[];\n  icon: LucideIcon;\n  shortcut?: string;\n  run: () => void;\n};\n\nexport type CommandMenuProps = {\n  open: boolean;\n  onOpenChange: (open: boolean) => void;\n};`,
    ),
    file(
      "src/components/command-menu/commands.ts",
      "",
      `import { Home, Settings, Users } from 'lucide-react';\nimport type { Command } from './types';\n\nexport const commands: Command[] = [\n  { id: 'home', label: 'Go to overview', icon: Home,\n    shortcut: 'G H', run: () => location.assign('/') },\n  { id: 'team', label: 'Open team', icon: Users,\n    keywords: ['members'], run: () => location.assign('/team') },\n  { id: 'settings', label: 'Open settings', icon: Settings,\n    run: () => location.assign('/settings') },\n];`,
    ),
    file(
      "src/components/command-menu/command-menu.css",
      ".command-search { display: flex; }",
      `.command-search {\n  display: flex;\n  align-items: center;\n  gap: 12px;\n  padding: 16px;\n  border-bottom: 1px solid var(--border);\n}\n\n[role="option"][aria-selected="true"] {\n  background: var(--accent-subtle);\n  outline: 2px solid var(--accent);\n  outline-offset: -2px;\n}`,
    ),
    file(
      "src/app/layout.tsx",
      `import { Header } from '@/components/header';\n\nexport default function Layout({ children }) {\n  return <><Header />{children}</>;\n}`,
      `import { Header } from '@/components/header';\nimport { CommandMenuProvider } from '@/providers/command-menu';\n\nexport default function Layout({ children }) {\n  return <CommandMenuProvider><Header />{children}</CommandMenuProvider>;\n}`,
    ),
    ...["command-menu", "keyboard-navigation", "filter-commands"].map((name) =>
      file(
        `src/components/command-menu/__tests__/${name}.test.tsx`,
        "",
        `import { describe, it, expect } from 'vitest';\nimport { filterCommands } from '../filter-commands';\n\ndescribe('${name}', () => {\n  it('returns no matches for an unknown command', () => {\n    const commands = [{ id: 'home', label: 'Home', run() {} }];\n    expect(filterCommands(commands, 'unknown')).toEqual([]);\n  });\n});`,
      ),
    ),
    file(
      "e2e/command-menu.spec.ts",
      "",
      `import { test, expect } from '@playwright/test';\n\ntest('opens the command menu with a shortcut', async ({ page }) => {\n  await page.goto('/');\n  await page.keyboard.press('ControlOrMeta+k');\n  await expect(page.getByRole('combobox')).toBeVisible();\n});`,
    ),
    file(
      "docs/components/command-menu.md",
      "",
      "# Command menu\n\nPress ⌘K to open the menu.\n\nUse the arrow keys to move through results, Enter to select, and Escape to dismiss.\n\nCommands are matched by label and keywords.",
    ),
    file(
      "docs/accessibility.md",
      "# Accessibility\n\nAll interactive elements need visible focus.",
      "# Accessibility\n\nAll interactive elements need visible focus.\n\nThe command menu uses the combobox pattern with an active descendant.",
    ),
    file(
      "README.md",
      "# Atlas\n\nA workspace for your team.",
      "# Atlas\n\nA workspace for your team.\n\n## Keyboard shortcuts\n\nOpen the command menu with ⌘K.",
    ),
    file(
      "package.json",
      '{\n  "name": "atlas",\n  "version": "1.0.0"\n}',
      '{\n  "name": "atlas",\n  "version": "1.1.0",\n  "scripts": { "test:e2e": "playwright test" }\n}',
    ),
    file(
      "vitest.config.ts",
      'export default { test: { environment: "node" } };',
      'export default { test: { environment: "jsdom", setupFiles: ["./tests/setup.ts"] } };',
    ),
    file(
      "public/icons/command.svg",
      "",
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">\n  <path d="M8 8h8v8H8z" fill="none" stroke="currentColor"/>\n</svg>',
    ),
    file(
      "public/icons/keyboard.svg",
      "",
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">\n  <rect x="2" y="5" width="20" height="14" rx="2"/>\n</svg>',
    ),
  ],
  threads: [
    {
      id: "sample-thread",
      path: "src/components/command-menu/use-keyboard-navigation.ts",
      line: 10,
      side: "RIGHT",
      resolved: false,
      canResolve: true,
      comments: [
        {
          id: "sample-comment",
          author: "alexrivera",
          body: "Could we also cover wrapping from the first result to the last? The empty-list guard looks good.",
          createdAt: "2026-10-13T09:00:00Z",
        },
      ],
    },
  ],
  conversation: [
    {
      id: "sample-conversation",
      author: "sarahchen",
      body: "Ready for review. I would especially appreciate a second look at the keyboard interactions.",
      createdAt: "2026-10-13T09:00:00Z",
    },
  ],
  commits: [
    {
      sha: "36fb81a",
      createdAt: "2026-10-12T08:30:00Z",
      message: "Add command menu primitives",
      author: "sarahchen",
    },
    {
      sha: "918b3d2",
      createdAt: "2026-10-12T11:00:00Z",
      message: "Support keyboard navigation and filtering",
      author: "sarahchen",
    },
    {
      sha: "a4f81d7",
      createdAt: "2026-10-13T08:00:00Z",
      message: "Add tests and document keyboard shortcuts",
      author: "sarahchen",
    },
  ],
};
