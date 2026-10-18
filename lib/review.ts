export type Category = "Code" | "Tests" | "Docs" | "Config" | "Assets";
export type Side = "LEFT" | "RIGHT";
export type DiffLine = {
  kind: "add" | "delete" | "context" | "hunk";
  text: string;
  old?: number;
  new?: number;
  hunk: number;
};
export type ChangedFile = {
  filename: string;
  previous_filename?: string;
  status: string;
  additions: number;
  deletions: number;
  patch?: string;
  sha: string;
  viewed?: boolean;
  content?: string;
  originalContent?: string;
};
export type Comment = {
  id: number | string;
  body: string;
  author: string;
  createdAt: string;
  url?: string;
  pending?: boolean;
  reviewId?: string;
};
export type Thread = {
  id: string;
  path: string;
  line: number | null;
  startLine?: number | null;
  side: Side;
  resolved?: boolean;
  outdated?: boolean;
  canResolve?: boolean;
  comments: Comment[];
  pending?: boolean;
};
export type Review = { id: string; state: string; body: string };
export type PullRequest = {
  owner: string;
  repo: string;
  number: number;
  id: string;
  title: string;
  body: string;
  author: string;
  state: string;
  createdAt?: string;
  updatedAt?: string;
  head: string;
  base: string;
  headRepo?: string;
  baseRepo?: string;
  headRef: string;
  baseRef: string;
  url: string;
  files: ChangedFile[];
  threads: Thread[];
  conversation: Comment[];
  reviews?: (Comment & { state: string; submittedAt?: string; commit?: string })[];
  commits: { sha: string; message: string; author: string; createdAt?: string; url?: string }[];
  pendingReview?: Review;
  viewer?: string;
  warnings: string[];
  demo: boolean;
};
export type ContextItem = {
  id: string;
  path: string;
  label: string;
  content: string;
  commit: string;
  side: Side;
  start?: number;
  end?: number;
  kind: "lines" | "diff" | "file";
};
export type Selection = {
  path: string;
  side: Side;
  start: number;
  end: number;
  hunk: number;
};
export const categories: Category[] = [
  "Code",
  "Tests",
  "Docs",
  "Config",
  "Assets",
];
export function categorize(path: string): Category {
  if (
    /(^|\/)(__tests__|tests?|specs?|e2e|fixtures|__snapshots__)(\/|$)|[.\-](test|spec|stories|snap)\./i.test(
      path,
    ) ||
    /(?:Test|Tests|Spec)\.(?:kt|swift|java)$/.test(path)
  )
    return "Tests";
  if (/(^|\/)(docs?|documentation)(\/|$)|\.(md|mdx|rst|txt)$/i.test(path))
    return "Docs";
  if (/\.(svg|png|jpe?g|gif|webp|ico|woff2?|ttf|mp4|pdf)$/i.test(path))
    return "Assets";
  if (
    /(^|\/)(\.[^/]+|package\.json|.*lock.*|tsconfig.*|.*\.config\.[^/]+)$|\.(json|ya?ml|toml|ini|env)$/i.test(
      path,
    )
  )
    return "Config";
  return "Code";
}
export function parsePatch(patch = ""): DiffLine[] {
  let old = 0,
    current = 0,
    hunk = -1;
  const result: DiffLine[] = [];
  for (const line of patch.split("\n")) {
    const header = line.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
    if (header) {
      old = Number(header[1]);
      current = Number(header[2]);
      hunk++;
      result.push({ kind: "hunk", text: line, hunk });
    } else if (hunk >= 0 && line.startsWith("+"))
      result.push({ kind: "add", text: line.slice(1), new: current++, hunk });
    else if (hunk >= 0 && line.startsWith("-"))
      result.push({ kind: "delete", text: line.slice(1), old: old++, hunk });
    else if (hunk >= 0 && line.startsWith(" "))
      result.push({
        kind: "context",
        text: line.slice(1),
        old: old++,
        new: current++,
        hunk,
      });
  }
  return result;
}
export function selectedLines(
  file: ChangedFile,
  selection: Selection,
): DiffLine[] {
  return parsePatch(file.patch).filter((line) => {
    const n = selection.side === "LEFT" ? line.old : line.new;
    return (
      line.hunk === selection.hunk &&
      n !== undefined &&
      n >= selection.start &&
      n <= selection.end
    );
  });
}
export function extendSelection(
  previous: Selection | null,
  path: string,
  side: Side,
  line: number,
  hunk: number,
  extend: boolean,
): Selection {
  if (
    extend &&
    previous?.path === path &&
    previous.side === side &&
    previous.hunk === hunk
  )
    return {
      path,
      side,
      hunk,
      start: Math.min(previous.start, line),
      end: Math.max(previous.start, line),
    };
  return { path, side, hunk, start: line, end: line };
}
export function splitRows(
  lines: DiffLine[],
): { left?: DiffLine; right?: DiffLine; hunk?: DiffLine }[] {
  const rows: { left?: DiffLine; right?: DiffLine; hunk?: DiffLine }[] = [];
  for (let i = 0; i < lines.length; ) {
    const line = lines[i];
    if (line.kind === "hunk") {
      rows.push({ hunk: line });
      i++;
    } else if (line.kind === "context") {
      rows.push({ left: line, right: line });
      i++;
    } else {
      const left: DiffLine[] = [],
        right: DiffLine[] = [];
      while (i < lines.length && lines[i].kind === "delete")
        left.push(lines[i++]);
      while (i < lines.length && lines[i].kind === "add")
        right.push(lines[i++]);
      for (let j = 0; j < Math.max(left.length, right.length); j++)
        rows.push({ left: left[j], right: right[j] });
    }
  }
  return rows;
}
export function parsePullUrl(value: string) {
  const url = new URL(value.trim());
  const match = url.pathname.match(
    /^\/([\w.-]+)\/([\w.-]+)\/pull\/([1-9]\d*)(?:\/|$)/,
  );
  if (
    url.protocol !== "https:" ||
    url.hostname !== "github.com" ||
    url.port ||
    url.username ||
    url.password ||
    !match
  )
    throw new Error(
      "Enter a GitHub pull request URL, such as https://github.com/owner/repo/pull/123.",
    );
  return { owner: match[1], repo: match[2], number: Number(match[3]) };
}
