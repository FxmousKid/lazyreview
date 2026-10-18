import { parsePullUrl, parsePatch } from "./review.ts";
import type { DiffLine, PullRequest, Thread } from "./review.ts";

export class ApiError extends Error {
  status: number;
  constructor(message: string, status = 400) {
    super(message);
    this.status = status;
  }
}
type Data = Record<string, any>;
type Fetcher = typeof fetch;
const comment = (c: Data) => ({
  id: c.databaseId ?? c.id,
  body: c.body,
  author: c.author?.login ?? c.user?.login ?? "ghost",
  createdAt: c.createdAt ?? c.created_at,
  url: c.html_url ?? c.url,
  pending: c.pullRequestReview?.state === "PENDING",
  reviewId: c.pullRequestReview?.id,
});
const threadFields = `id path line startLine diffSide isResolved isOutdated viewerCanResolve viewerCanUnresolve comments(first:100){nodes{id databaseId body createdAt url author{login} pullRequestReview{id state}} pageInfo{hasNextPage endCursor}}`;

export async function executeGithub(
  input: Data,
  token: string,
  fetcher: Fetcher = fetch,
): Promise<any> {
  const { owner, repo, number } = parsePullUrl(String(input.url || ""));
  const root = `/repos/${owner}/${repo}`;
  const pull = `${root}/pulls/${number}`;
  const operation = input.operation;
  const write = [
    "viewed",
    "draft",
    "submit",
    "reply",
    "resolve",
    "conversation",
  ].includes(operation);
  if (write && !token)
    throw new ApiError(
      "Connect a GitHub token to perform review actions.",
      401,
    );
  async function request(
    path: string,
    method = "GET",
    body?: unknown,
    envelope = false,
  ): Promise<any> {
    let response: Response;
    try {
      response = await fetcher(`https://api.github.com${path}`, {
        method,
        headers: {
          Accept: "application/vnd.github+json",
          "X-GitHub-Api-Version": "2026-03-10",
          "User-Agent": "lazyreview-poc",
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
          ...(body ? { "Content-Type": "application/json" } : {}),
        },
        body: body ? JSON.stringify(body) : undefined,
        signal: AbortSignal.timeout(25000),
        cache: "no-store",
      });
    } catch {
      throw new ApiError(
        write
          ? "The request did not complete. Its outcome may be uncertain; refresh the PR before trying again."
          : "GitHub did not respond. Try refreshing.",
        502,
      );
    }
    if (!response.ok) {
      const reset = response.headers.get("x-ratelimit-reset");
      const reason =
        response.status === 401
          ? "GitHub did not accept the token."
          : response.status === 404
            ? "The PR or file is unavailable. Check its URL and your repository access."
            : response.status === 403 || response.status === 429
              ? `GitHub denied this request. Check repository permissions or rate limits.${reset && response.headers.get("x-ratelimit-remaining") === "0" ? ` Rate limit resets at ${new Date(Number(reset) * 1000).toISOString()}.` : ""}`
              : response.status === 422
                ? "GitHub could not accept this action. Refresh and check the line selection, pending review, and review permissions."
                : `GitHub returned HTTP ${response.status}. Refresh to check the current state.`;
      throw new ApiError(reason, response.status);
    }
    const data = response.status === 204 ? {} : await response.json();
    return envelope
      ? { data, next: /rel="next"/.test(response.headers.get("link") || "") }
      : data;
  }
  async function graphql(query: string, variables: Data): Promise<Data> {
    const data = await request("/graphql", "POST", { query, variables });
    if (data.errors?.length)
      throw new ApiError(
        write
          ? "GitHub rejected the review action. Refresh to reconcile any pending changes, then check permissions."
          : "Some GitHub review data could not be read. Check token permissions.",
        422,
      );
    return data.data;
  }
  async function all(path: string, limit = 30) {
    const result: any[] = [];
    for (let page = 1; page <= limit; page++) {
      const response = await request(
        `${path}${path.includes("?") ? "&" : "?"}per_page=100&page=${page}`,
        "GET",
        undefined,
        true,
      );
      result.push(...response.data);
      if (!response.next) return result;
    }
    throw new ApiError(
      "This PR exceeds the proof of concept’s pagination limit. Open it on GitHub to see the complete review.",
      422,
    );
  }
  if (operation === "load") {
    const pr = await request(pull);
    if (pr.changed_files > 3000)
      throw new ApiError(
        "GitHub’s file API caps responses at 3,000 files. This PR is too large to load completely here.",
        422,
      );
    const [files, rawComments, conversation, commits, comparison, reviews] =
      await Promise.all([
        all(`${pull}/files`),
        all(`${pull}/comments`),
        all(`${root}/issues/${number}/comments`),
        all(`${pull}/commits`, 4),
        request(`${root}/compare/${pr.base.sha}...${pr.head.sha}?per_page=1`),
        all(`${pull}/reviews`),
      ]);
    const warnings: string[] = [];
    if (files.length !== pr.changed_files)
      throw new ApiError(
        "GitHub returned an incomplete file list. Refresh the PR.",
        409,
      );
    let threads: Thread[] = [],
      pendingReview;
    let viewer: string | undefined;
    if (token) {
      let cursor: string | null = null;
      do {
        const data = await graphql(
          `query($owner:String!,$repo:String!,$number:Int!,$cursor:String){viewer{login} repository(owner:$owner,name:$repo){pullRequest(number:$number){headRefOid reviewThreads(first:100,after:$cursor){nodes{${threadFields}} pageInfo{hasNextPage endCursor}} reviews(first:100,states:[PENDING]){nodes{id state body author{login}}}}}}`,
          { owner, repo, number, cursor },
        );
        const p = data.repository.pullRequest;
        if (p.headRefOid !== pr.head.sha)
          throw new ApiError(
            "New commits arrived while loading. Refresh to load a consistent version.",
            409,
          );
        viewer = data.viewer.login;
        pendingReview = p.reviews.nodes.find(
          (r: Data) => r.author?.login === viewer,
        );
        for (const t of p.reviewThreads.nodes) {
          let nodes = t.comments.nodes;
          let page = t.comments.pageInfo;
          while (page.hasNextPage) {
            const next = await graphql(
              `
                query ($id: ID!, $cursor: String!) {
                  node(id: $id) {
                    ... on PullRequestReviewThread {
                      comments(first: 100, after: $cursor) {
                        nodes {
                          id
                          databaseId
                          body
                          createdAt
                          url
                          author {
                            login
                          }
                          pullRequestReview {
                            id
                            state
                          }
                        }
                        pageInfo {
                          hasNextPage
                          endCursor
                        }
                      }
                    }
                  }
                }
              `,
              { id: t.id, cursor: page.endCursor },
            );
            nodes = [...nodes, ...next.node.comments.nodes];
            page = next.node.comments.pageInfo;
          }
          threads.push({
            id: t.id,
            path: t.path,
            line: t.line,
            startLine: t.startLine,
            side: t.diffSide,
            resolved: t.isResolved,
            outdated: t.isOutdated,
            canResolve: t.isResolved
              ? t.viewerCanUnresolve
              : t.viewerCanResolve,
            pending: nodes[0]?.pullRequestReview?.state === "PENDING",
            comments: nodes.map(comment),
          });
        }
        cursor = p.reviewThreads.pageInfo.hasNextPage
          ? p.reviewThreads.pageInfo.endCursor
          : null;
      } while (cursor);
      cursor = null;
      do {
        const data = await graphql(
          `
            query (
              $owner: String!
              $repo: String!
              $number: Int!
              $cursor: String
            ) {
              repository(owner: $owner, name: $repo) {
                pullRequest(number: $number) {
                  files(first: 100, after: $cursor) {
                    nodes {
                      path
                      viewerViewedState
                    }
                    pageInfo {
                      hasNextPage
                      endCursor
                    }
                  }
                }
              }
            }
          `,
          { owner, repo, number, cursor },
        );
        const page = data.repository.pullRequest.files;
        for (const f of page.nodes) {
          const target = files.find((x) => x.filename === f.path);
          if (target) target.viewed = f.viewerViewedState === "VIEWED";
        }
        cursor = page.pageInfo.hasNextPage ? page.pageInfo.endCursor : null;
      } while (cursor);
    } else {
      const roots = rawComments.filter((c) => !c.in_reply_to_id);
      threads = roots.map((c) => ({
        id: String(c.id),
        path: c.path,
        line: c.line,
        startLine: c.start_line,
        side: c.side || "RIGHT",
        outdated: c.line === null,
        comments: [
          c,
          ...rawComments.filter((r) => r.in_reply_to_id === c.id),
        ].map(comment),
      }));
      warnings.push(
        "Public preview: connect GitHub to load your pending review, viewed files, and thread resolution.",
      );
    }
    if (commits.length < pr.commits)
      warnings.push(
        "GitHub limits commit history returned by this endpoint; additional commits are available on GitHub.",
      );
    const finalPr = await request(pull);
    if (finalPr.head.sha !== pr.head.sha || finalPr.base.sha !== pr.base.sha)
      throw new ApiError(
        "The PR changed while loading. Refresh to load a consistent version.",
        409,
      );
    return {
      owner,
      repo,
      number,
      id: pr.node_id,
      title: pr.title,
      body: pr.body || "",
      author: pr.user.login,
      createdAt: pr.created_at,
      updatedAt: pr.updated_at,
      state: pr.merged ? "merged" : pr.state,
      head: pr.head.sha,
      base: comparison.merge_base_commit.sha,
      headRepo: pr.head.repo?.full_name || `${owner}/${repo}`,
      baseRepo: pr.base.repo.full_name,
      headRef: pr.head.ref,
      baseRef: pr.base.ref,
      url: pr.html_url,
      files,
      threads,
      conversation: conversation.map(comment),
      reviews: reviews.filter((r) => r.state !== "PENDING" && r.submitted_at).map((r) => ({
        id: r.id,
        body: r.body || "",
        author: r.user?.login || "ghost",
        state: r.state,
        createdAt: r.submitted_at,
        submittedAt: r.submitted_at,
        url: r.html_url,
        commit: r.commit_id,
      })),
      commits: commits.map((c) => ({
        sha: c.sha,
        message: c.commit.message.split("\n")[0],
        author: c.author?.login ?? c.commit.author.name,
        createdAt: c.commit.committer?.date ?? c.commit.author?.date,
        url: c.html_url,
      })),
      pendingReview,
      viewer,
      warnings,
      demo: false,
    } satisfies PullRequest;
  }
  const pr = await request(pull);
  if (input.expectedHead !== pr.head.sha)
    throw new ApiError(
      "New commits were pushed. Refresh and reselect the lines before continuing; your draft has been preserved.",
      409,
    );
  async function verifySnapshot() {
    const current = await request(pull);
    if (current.head.sha !== input.expectedHead)
      throw new ApiError(
        "New commits were pushed. Refresh before continuing; your draft has been preserved.",
        409,
      );
    const comparison = await request(
      `${root}/compare/${current.base.sha}...${current.head.sha}?per_page=1`,
    );
    if (comparison.merge_base_commit.sha !== input.expectedBase)
      throw new ApiError(
        "The comparison base changed. Refresh and reselect the lines; your draft has been preserved.",
        409,
      );
  }
  if (write) await verifySnapshot();
  if (operation === "content") {
    if (!["LEFT", "RIGHT"].includes(input.side))
      throw new ApiError("Choose an original or modified file.");
    const files = await all(`${pull}/files`);
    const file = files.find((f) => f.filename === input.path);
    if (!file) throw new ApiError("This file does not belong to the PR.");
    if (input.side === "RIGHT" && file.status === "removed")
      throw new ApiError(
        "This file was deleted. Attach its original version instead.",
      );
    if (input.side === "LEFT" && file.status === "added")
      throw new ApiError("This file is new and has no original version.");
    const comparison =
      input.side === "LEFT"
        ? await request(
            `${root}/compare/${pr.base.sha}...${pr.head.sha}?per_page=1`,
          )
        : null;
    const ref =
      input.side === "LEFT" ? comparison.merge_base_commit.sha : pr.head.sha;
    if (input.side === "LEFT" && ref !== input.expectedBase)
      throw new ApiError(
        "The original side of the diff changed. Refresh before attaching this file.",
        409,
      );
    const repository =
      input.side === "LEFT" ? pr.base.repo.full_name : pr.head.repo?.full_name;
    if (!repository)
      throw new ApiError("The source repository is no longer available.");
    const path =
      input.side === "LEFT"
        ? file.previous_filename || file.filename
        : file.filename;
    const data = await request(
      `/repos/${repository}/contents/${path.split("/").map(encodeURIComponent).join("/")}?ref=${ref}`,
    );
    if (data.encoding !== "base64" || !data.content || data.size > 200000)
      throw new ApiError(
        "This file is binary, empty, or too large to attach. Select a diff range instead.",
      );
    const content = new TextDecoder("utf-8", { fatal: true }).decode(
      Uint8Array.from(atob(data.content.replace(/\s/g, "")), (c) =>
        c.charCodeAt(0),
      ),
    );
    if (content.includes("\0"))
      throw new ApiError("Binary files cannot be attached to chat.");
    return { content, commit: ref };
  }
  if (!write) throw new ApiError("Unknown review action.");
  const mutation = async (
    name: string,
    type: string,
    value: Data,
    fields: string,
  ) => {
    await verifySnapshot();
    return graphql(
      `mutation($input:${type}!){${name}(input:$input){${fields}}}`,
      { input: value },
    );
  };
  const body = String(input.body || "").trim();
  if (operation === "submit" && body.length > 60000)
    throw new ApiError("Keep the review summary within 60,000 characters.");
  if (
    ["draft", "reply", "conversation"].includes(operation) &&
    (!body || body.length > 60000)
  )
    throw new ApiError("Write a comment between 1 and 60,000 characters.");
  if (operation === "viewed") {
    const files = await all(`${pull}/files`);
    if (!files.some((f) => f.filename === input.path))
      throw new ApiError("This file does not belong to the PR.");
    return mutation(
      input.viewed ? "markFileAsViewed" : "unmarkFileAsViewed",
      input.viewed ? "MarkFileAsViewedInput" : "UnmarkFileAsViewedInput",
      { pullRequestId: pr.node_id, path: input.path },
      "pullRequest{id}",
    );
  }
  if (operation === "draft") {
    if (
      !["LEFT", "RIGHT"].includes(input.side) ||
      !Number.isInteger(input.line) ||
      input.line < 1 ||
      !Number.isInteger(input.startLine) ||
      input.startLine < 1 ||
      input.startLine > input.line
    )
      throw new ApiError(
        "Select a valid contiguous range on one side of the diff.",
      );
    const files = await all(`${pull}/files`);
    const file = files.find((f) => f.filename === input.path);
    const eligible: DiffLine[] | undefined = file
      ? parsePatch(file.patch).filter((l) => {
          const n = input.side === "LEFT" ? l.old : l.new;
          return n !== undefined && n >= input.startLine && n <= input.line;
        })
      : undefined;
    if (
      !eligible?.length ||
      eligible.length !== input.line - input.startLine + 1 ||
      new Set(eligible.map((l) => l.hunk)).size !== 1
    )
      throw new ApiError(
        "The selected range must be in one returned diff hunk.",
      );
    const data = await graphql(
      `
        query ($owner: String!, $repo: String!, $number: Int!) {
          viewer {
            login
          }
          repository(owner: $owner, name: $repo) {
            pullRequest(number: $number) {
              reviews(first: 100, states: [PENDING]) {
                nodes {
                  id
                  author {
                    login
                  }
                  commit {
                    oid
                  }
                }
              }
            }
          }
        }
      `,
      { owner, repo, number },
    );
    const existing = data.repository.pullRequest.reviews.nodes.find(
      (r: Data) => r.author?.login === data.viewer.login,
    );
    if (existing?.commit?.oid && existing.commit.oid !== pr.head.sha)
      throw new ApiError(
        "Your pending review belongs to an older commit. Finish or discard that review on GitHub, then refresh before adding new line comments.",
        409,
      );
    let reviewId = existing?.id;
    if (!reviewId) {
      const created = await mutation(
        "addPullRequestReview",
        "AddPullRequestReviewInput",
        { pullRequestId: pr.node_id, commitOID: pr.head.sha },
        "pullRequestReview{id}",
      );
      reviewId = created.addPullRequestReview.pullRequestReview.id;
    }
    return mutation(
      "addPullRequestReviewThread",
      "AddPullRequestReviewThreadInput",
      {
        pullRequestReviewId: reviewId,
        body,
        path: input.path,
        line: input.line,
        side: input.side,
        ...(input.startLine < input.line
          ? { startLine: input.startLine, startSide: input.side }
          : {}),
      },
      "thread{id}",
    );
  }
  if (operation === "submit") {
    if (!["APPROVE", "COMMENT", "REQUEST_CHANGES"].includes(input.event))
      throw new ApiError("Choose a valid review action.");
    const data = await graphql(
      `
        query ($owner: String!, $repo: String!, $number: Int!) {
          viewer {
            login
          }
          repository(owner: $owner, name: $repo) {
            pullRequest(number: $number) {
              reviews(first: 100, states: [PENDING]) {
                nodes {
                  id
                  author {
                    login
                  }
                  commit {
                    oid
                  }
                }
              }
            }
          }
        }
      `,
      { owner, repo, number },
    );
    if (pr.user.login === data.viewer.login && input.event !== "COMMENT")
      throw new ApiError(
        "You cannot approve or request changes on your own PR.",
      );
    const review = data.repository.pullRequest.reviews.nodes.find(
      (r: Data) => r.author?.login === data.viewer.login,
    );
    if (review?.commit?.oid && review.commit.oid !== pr.head.sha)
      throw new ApiError(
        "Your pending review belongs to an older commit. Finish or discard it on GitHub before reviewing this snapshot.",
        409,
      );
    if (review)
      return mutation(
        "submitPullRequestReview",
        "SubmitPullRequestReviewInput",
        {
          pullRequestReviewId: review.id,
          event: input.event,
          ...(body ? { body } : {}),
        },
        "pullRequestReview{id state}",
      );
    if (input.event !== "APPROVE" && !body)
      throw new ApiError(
        "Add a review summary or a pending comment before submitting.",
      );
    await verifySnapshot();
    return request(`${pull}/reviews`, "POST", {
      commit_id: pr.head.sha,
      body,
      event: input.event,
    });
  }
  if (operation === "reply") {
    const comments = await all(`${pull}/comments`);
    if (!comments.some((c) => c.id === input.commentId && !c.in_reply_to_id))
      throw new ApiError("Refresh before replying to this thread.");
    await verifySnapshot();
    return request(`${pull}/comments/${input.commentId}/replies`, "POST", {
      body,
    });
  }
  if (operation === "conversation") {
    await verifySnapshot();
    return request(`${root}/issues/${number}/comments`, "POST", { body });
  }
  if (operation === "resolve") {
    const data = await graphql(
      `
        query ($id: ID!) {
          node(id: $id) {
            ... on PullRequestReviewThread {
              id
              pullRequest {
                id
              }
            }
          }
        }
      `,
      { id: input.threadId },
    );
    if (data.node?.pullRequest?.id !== pr.node_id)
      throw new ApiError("This thread does not belong to the current PR.");
    return mutation(
      input.resolved ? "resolveReviewThread" : "unresolveReviewThread",
      input.resolved
        ? "ResolveReviewThreadInput"
        : "UnresolveReviewThreadInput",
      { threadId: input.threadId },
      "thread{id isResolved}",
    );
  }
}
