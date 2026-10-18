import test from "node:test";
import assert from "node:assert/strict";
import {
  categorize,
  parsePatch,
  extendSelection,
  selectedLines,
  splitRows,
  parsePullUrl,
} from "../lib/review.ts";
import { executeGithub } from "../lib/github.ts";
const patch =
  "@@ -9,3 +9,4 @@ function run()\n unchanged\n-old\n+new\n+extra\n end\n@@ -30 +31 @@\n-other\n+replacement";
const file = {
  filename: "src/a.ts",
  status: "modified",
  additions: 3,
  deletions: 2,
  patch,
  sha: "f",
};
const url = "https://github.com/acme/atlas/pull/128";
const pr = {
  node_id: "PR_1",
  changed_files: 1,
  title: "Example",
  body: "",
  state: "open",
  user: { login: "author" },
  head: { sha: "head", ref: "feature", repo: { full_name: "fork/atlas" } },
  base: { sha: "base-tip", ref: "main", repo: { full_name: "acme/atlas" } },
  html_url: url,
  commits: 0,
};
function fake(handler: (url: string, init: RequestInit) => unknown) {
  return (async (url: any, init: any) => {
    if (String(url).endsWith("/graphql")) {
      const query = JSON.parse(init.body).query;
      assert.equal(
        (query.match(/\{/g) || []).length,
        (query.match(/\}/g) || []).length,
        "balanced GraphQL query",
      );
    }
    const data = String(url).includes("/compare/")
      ? { merge_base_commit: { sha: "merge-base" } }
      : handler(String(url), init || {});
    return data instanceof Response ? data : Response.json(data);
  }) as typeof fetch;
}
test("cohorts distinguish colocated tests, docs, assets, config and source", () => {
  assert.equal(categorize("src/Button.test.tsx"), "Tests");
  assert.equal(categorize("app/FooTests.swift"), "Tests");
  assert.equal(categorize("src/__snapshots__/foo.snap"), "Tests");
  assert.equal(categorize("docs/plan.ts"), "Docs");
  assert.equal(categorize("vitest.config.ts"), "Config");
  assert.equal(categorize("public/icon.svg"), "Assets");
  assert.equal(categorize("src/Button.tsx"), "Code");
});
test("patch numbering preserves deleted and added sides across hunks", () => {
  const lines = parsePatch(patch);
  assert.equal(lines[2].old, 10);
  assert.equal(lines[2].new, undefined);
  assert.equal(lines[3].new, 10);
  assert.equal(lines[4].new, 11);
  assert.equal(lines.at(-1)?.new, 31);
  assert.equal(lines.at(-1)?.hunk, 1);
});
test("selection cannot jump between files, sides or hunks", () => {
  const first = extendSelection(null, file.filename, "RIGHT", 10, 0, false);
  const range = extendSelection(first, file.filename, "RIGHT", 11, 0, true);
  assert.equal(
    selectedLines(file, range)
      .map((l) => l.text)
      .join("\n"),
    "new\nextra",
  );
  assert.equal(
    extendSelection(range, file.filename, "LEFT", 10, 0, true).start,
    10,
  );
  assert.equal(
    extendSelection(range, file.filename, "RIGHT", 31, 1, true).start,
    31,
  );
  assert.equal(
    extendSelection(range, "other.ts", "RIGHT", 5, 0, true).path,
    "other.ts",
  );
});
test("split view aligns removal and additions without losing actual line numbers", () => {
  const rows = splitRows(parsePatch(patch));
  assert.equal(rows[2].left?.old, 10);
  assert.equal(rows[2].right?.new, 10);
  assert.equal(rows[3].left, undefined);
  assert.equal(rows[3].right?.new, 11);
});
test("PR URL rejects arbitrary hosts, credentials and non-PR paths", () => {
  assert.deepEqual(parsePullUrl(url + "/changes"), {
    owner: "acme",
    repo: "atlas",
    number: 128,
  });
  for (const bad of [
    "https://github.com.evil.test/a/b/pull/1",
    "https://user:password@github.com/a/b/pull/1",
    "http://github.com/a/b/pull/1",
    "https://github.com/a/b/issues/1",
  ])
    assert.throws(() => parsePullUrl(bad));
});
test("head drift rejects mutations before any write", async () => {
  const methods: string[] = [];
  const fetcher = fake((_, init) => {
    methods.push(init.method!);
    return pr;
  });
  await assert.rejects(
    executeGithub(
      { operation: "draft", url, expectedHead: "stale" },
      "token",
      fetcher,
    ),
    /New commits/,
  );
  assert.deepEqual(methods, ["GET"]);
});
test("missing credentials reject mutations without any upstream call", async () => {
  await assert.rejects(
    executeGithub(
      {
        operation: "submit",
        url,
        expectedHead: "head",
        expectedBase: "merge-base",
        event: "APPROVE",
      },
      "",
      fake(() => assert.fail("must not fetch")),
    ),
    /Connect a GitHub token/,
  );
});
test("pending range comment reuses review and sends immutable range coordinates", async () => {
  const writes: any[] = [];
  await executeGithub(
    {
      operation: "draft",
      url,
      expectedHead: "head",
      expectedBase: "merge-base",
      path: file.filename,
      side: "RIGHT",
      startLine: 10,
      line: 11,
      body: "Check this range",
    },
    "token",
    fake((path, init) => {
      if (path.includes("/graphql")) {
        const body = JSON.parse(init.body as string);
        if (body.query.trimStart().startsWith("query"))
          return {
            data: {
              viewer: { login: "reviewer" },
              repository: {
                pullRequest: {
                  reviews: {
                    nodes: [{ id: "REVIEW_1", author: { login: "reviewer" } }],
                  },
                },
              },
            },
          };
        writes.push(body);
        return {
          data: { addPullRequestReviewThread: { thread: { id: "THREAD_1" } } },
        };
      }
      if (path.includes("/files")) return [file];
      return pr;
    }),
  );
  assert.equal(writes.length, 1);
  assert.deepEqual(writes[0].variables.input, {
    pullRequestReviewId: "REVIEW_1",
    body: "Check this range",
    path: file.filename,
    line: 11,
    side: "RIGHT",
    startLine: 10,
    startSide: "RIGHT",
  });
});
test("range outside a returned patch cannot produce a comment", async () => {
  await assert.rejects(
    executeGithub(
      {
        operation: "draft",
        url,
        expectedHead: "head",
        expectedBase: "merge-base",
        path: file.filename,
        side: "RIGHT",
        startLine: 10,
        line: 31,
        body: "Bad range",
      },
      "token",
      fake((path) => (path.includes("/files") ? [file] : pr)),
    ),
    /one returned diff hunk/,
  );
});
test("deleted or renamed original file uses merge-base and old filename", async () => {
  let contentUrl = "";
  const result = await executeGithub(
    {
      operation: "content",
      url,
      expectedHead: "head",
      expectedBase: "merge-base",
      path: "src/new.ts",
      side: "LEFT",
    },
    "token",
    fake((path) => {
      if (path.includes("/files"))
        return [
          {
            ...file,
            filename: "src/new.ts",
            previous_filename: "src/old.ts",
            status: "renamed",
          },
        ];
      if (path.includes("/compare/"))
        return { merge_base_commit: { sha: "merge-base" } };
      if (path.includes("/contents/")) {
        contentUrl = path;
        return { encoding: "base64", size: 3, content: btoa("old") };
      }
      return pr;
    }),
  );
  assert.equal(result.content, "old");
  assert.match(
    contentUrl,
    /acme\/atlas\/contents\/src\/old.ts\?ref=merge-base$/,
  );
});
test("whole modified file reads the fork at the pinned head", async () => {
  let contentUrl = "";
  await executeGithub(
    {
      operation: "content",
      url,
      expectedHead: "head",
      expectedBase: "merge-base",
      path: file.filename,
      side: "RIGHT",
    },
    "",
    fake((path) => {
      if (path.includes("/files")) return [file];
      if (path.includes("/contents/")) {
        contentUrl = path;
        return { encoding: "base64", size: 3, content: btoa("new") };
      }
      return pr;
    }),
  );
  assert.match(contentUrl, /fork\/atlas\/contents\/src\/a.ts\?ref=head$/);
});
test("file pagination loads all pages and retains the merge-base snapshot", async () => {
  const files = Array.from({ length: 101 }, (_, i) => ({
    ...file,
    filename: `src/${i}.ts`,
  }));
  const result = await executeGithub(
    { operation: "load", url },
    "",
    fake((path) => {
      if (path.includes("/files"))
        return path.includes("page=2")
          ? files.slice(100)
          : Response.json(files.slice(0, 100), {
              headers: { link: '<https://api.github.com/page2>; rel="next"' },
            });
      if (path.includes("/compare/"))
        return { merge_base_commit: { sha: "merge-base" } };
      if (path.includes("/comments") || path.includes("/commits") || path.includes("/reviews")) return [];
      return { ...pr, changed_files: 101 };
    }),
  );
  assert.equal(result.files.length, 101);
  assert.equal(result.base, "merge-base");
});
test("incomplete GitHub file list fails visibly", async () => {
  await assert.rejects(
    executeGithub(
      { operation: "load", url },
      "",
      fake((path) => {
        if (path.includes("/files")) return [];
        if (path.includes("/compare/"))
          return { merge_base_commit: { sha: "merge-base" } };
        if (path.includes("/comments") || path.includes("/commits") || path.includes("/reviews")) return [];
        return pr;
      }),
    ),
    /incomplete file list/,
  );
});
test("thread IDs cannot resolve a thread from a different pull request", async () => {
  await assert.rejects(
    executeGithub(
      {
        operation: "resolve",
        url,
        expectedHead: "head",
        expectedBase: "merge-base",
        threadId: "OTHER",
        resolved: true,
      },
      "token",
      fake((path) =>
        path.includes("/graphql")
          ? { data: { node: { pullRequest: { id: "PR_OTHER" } } } }
          : pr,
      ),
    ),
    /does not belong/,
  );
});
test("provider errors never leak raw upstream content or credentials", async () => {
  await assert.rejects(
    executeGithub(
      { operation: "load", url },
      "secret-token",
      fake(
        () =>
          new Response('{"message":"secret-token private internals"}', {
            status: 403,
          }),
      ),
    ),
    (error) => {
      assert.ok(error instanceof Error);
      assert.match(error.message, /permissions or rate limits/);
      assert.doesNotMatch(error.message, /secret-token|private internals/);
      return true;
    },
  );
});

test("base drift blocks review actions even if the head is unchanged", async () => {
  const fetcher = (async (path: any) =>
    Response.json(
      String(path).includes("/compare/")
        ? { merge_base_commit: { sha: "new-base" } }
        : pr,
    )) as typeof fetch;
  await assert.rejects(
    executeGithub(
      {
        operation: "viewed",
        url,
        expectedHead: "head",
        expectedBase: "old-base",
        path: file.filename,
        viewed: true,
      },
      "token",
      fetcher,
    ),
    /comparison base changed/,
  );
});
test("a full final page without a next Link is complete", async () => {
  const files = Array.from({ length: 100 }, (_, i) => ({
    ...file,
    filename: `src/${i}.ts`,
  }));
  let pages = 0;
  const result = await executeGithub(
    { operation: "load", url },
    "",
    fake((path) => {
      if (path.includes("/files")) {
        pages++;
        return files;
      }
      if (path.includes("/comments") || path.includes("/commits") || path.includes("/reviews")) return [];
      return { ...pr, changed_files: 100 };
    }),
  );
  assert.equal(result.files.length, 100);
  assert.equal(pages, 1);
});
test("new draft review is pinned to the head before adding the thread", async () => {
  const mutations: any[] = [];
  await executeGithub(
    {
      operation: "draft",
      url,
      expectedHead: "head",
      expectedBase: "merge-base",
      path: file.filename,
      side: "LEFT",
      startLine: 10,
      line: 10,
      body: "Deleted line",
    },
    "token",
    fake((path, init) => {
      if (path.includes("/files")) return [file];
      if (path.includes("/graphql")) {
        const body = JSON.parse(init.body as string);
        if (body.query.trimStart().startsWith("query"))
          return {
            data: {
              viewer: { login: "reviewer" },
              repository: { pullRequest: { reviews: { nodes: [] } } },
            },
          };
        mutations.push(body);
        return body.query.includes("addPullRequestReview(")
          ? {
              data: {
                addPullRequestReview: {
                  pullRequestReview: { id: "NEW_REVIEW" },
                },
              },
            }
          : {
              data: {
                addPullRequestReviewThread: { thread: { id: "THREAD" } },
              },
            };
      }
      return pr;
    }),
  );
  assert.equal(mutations.length, 2);
  assert.equal(mutations[0].variables.input.commitOID, "head");
  assert.equal(mutations[1].variables.input.pullRequestReviewId, "NEW_REVIEW");
  assert.equal(mutations[1].variables.input.side, "LEFT");
  assert.equal(mutations[1].variables.input.startLine, undefined);
});
test("submit uses the authenticated identity for self-review protection", async () => {
  await assert.rejects(
    executeGithub(
      {
        operation: "submit",
        url,
        expectedHead: "head",
        expectedBase: "merge-base",
        event: "APPROVE",
        viewer: "spoofed",
      },
      "token",
      fake((path) =>
        path.endsWith("/graphql")
          ? {
              data: {
                viewer: { login: "author" },
                repository: { pullRequest: { reviews: { nodes: [] } } },
              },
            }
          : pr,
      ),
    ),
    /your own PR/,
  );
});
test("an older pending review is not silently submitted against a new snapshot", async () => {
  await assert.rejects(
    executeGithub(
      {
        operation: "submit",
        url,
        expectedHead: "head",
        expectedBase: "merge-base",
        event: "APPROVE",
      },
      "token",
      fake((path) =>
        path.endsWith("/graphql")
          ? {
              data: {
                viewer: { login: "reviewer" },
                repository: {
                  pullRequest: {
                    reviews: {
                      nodes: [
                        {
                          id: "OLD",
                          author: { login: "reviewer" },
                          commit: { oid: "old-head" },
                        },
                      ],
                    },
                  },
                },
              },
            }
          : pr,
      ),
    ),
    /older commit/,
  );
});
test("submit sends pending review ID and explicit review event", async () => {
  let submitted: any;
  await executeGithub(
    {
      operation: "submit",
      url,
      expectedHead: "head",
      expectedBase: "merge-base",
      event: "REQUEST_CHANGES",
      body: "Please add tests.",
    },
    "token",
    fake((path, init) => {
      if (!path.endsWith("/graphql")) return pr;
      const body = JSON.parse(init.body as string);
      if (body.query.trimStart().startsWith("query"))
        return {
          data: {
            viewer: { login: "reviewer" },
            repository: {
              pullRequest: {
                reviews: {
                  nodes: [
                    {
                      id: "PENDING",
                      author: { login: "reviewer" },
                      commit: { oid: "head" },
                    },
                  ],
                },
              },
            },
          },
        };
      submitted = body.variables.input;
      return {
        data: {
          submitPullRequestReview: {
            pullRequestReview: { id: "PENDING", state: "CHANGES_REQUESTED" },
          },
        },
      };
    }),
  );
  assert.deepEqual(submitted, {
    pullRequestReviewId: "PENDING",
    event: "REQUEST_CHANGES",
    body: "Please add tests.",
  });
});

test('authenticated load includes pending replies and validates query syntax', async () => {
  const result = await executeGithub({ operation: 'load', url }, 'token', fake((path, init) => {
    if (path.endsWith('/graphql')) {
      const { query } = JSON.parse(init.body as string);
      if (query.includes('reviewThreads')) return { data: { viewer: { login: 'reviewer' }, repository: { pullRequest: { headRefOid: 'head', reviews: { nodes: [{ id: 'PENDING', author: { login: 'reviewer' } }] }, reviewThreads: { nodes: [{ id: 'THREAD', path: file.filename, line: 10, diffSide: 'RIGHT', isResolved: false, viewerCanResolve: true, comments: { nodes: [{ databaseId: 1, body: 'Published root', author: { login: 'author' }, pullRequestReview: { id: 'OLD', state: 'COMMENTED' } }, { databaseId: 2, body: 'My pending reply', author: { login: 'reviewer' }, pullRequestReview: { id: 'PENDING', state: 'PENDING' } }], pageInfo: { hasNextPage: false } } }], pageInfo: { hasNextPage: false } } } } } };
      return { data: { repository: { pullRequest: { files: { nodes: [{ path: file.filename, viewerViewedState: 'DISMISSED' }], pageInfo: { hasNextPage: false } } } } } };
    }
    if (path.includes('/files')) return [file];
    if (path.includes('/comments') || path.includes('/commits') || path.includes('/reviews')) return [];
    return pr;
  }));
  assert.equal(result.threads[0].pending, false);
  assert.equal(result.threads[0].comments[1].pending, true);
  assert.equal(result.threads[0].comments[1].reviewId, 'PENDING');
  assert.equal(result.files[0].viewed, false);
});

test('PR history preserves timestamps, paginates submitted reviews and excludes private drafts', async () => {
  const calls: string[] = [];
  const opened = '2026-10-12T08:00:00Z', submitted = '2026-10-13T09:00:00Z';
  const result = await executeGithub({operation:'load',url},'',fake((path,init)=>{
    calls.push(path);assert.equal(init.method,'GET');
    if(path.includes('/files'))return [file];
    if(path.includes('/reviews'))return path.includes('page=2')
      ? [{id:2,user:{login:'reviewer'},state:'APPROVED',body:'',submitted_at:submitted,html_url:url+'#pullrequestreview-2'}]
      : Response.json([{id:1,user:{login:'reviewer'},state:'COMMENTED',body:'[Build](https://example.org)',submitted_at:submitted,html_url:url+'#pullrequestreview-1'}, {id:3,user:{login:'viewer'},state:'PENDING',body:'private draft'}],{headers:{link:'<https://api.github.com/next>; rel="next"'}});
    if(path.includes('/commits'))return [{sha:'123',commit:{message:'First line\nDetails',author:{name:'author',date:opened},committer:{date:submitted}},html_url:'https://github.com/acme/atlas/commit/123'}];
    if(path.includes('/comments'))return [];
    return {...pr,created_at:opened,updated_at:submitted,commits:1};
  }));
  assert.equal(result.createdAt,opened);
  assert.equal(result.updatedAt,submitted);
  assert.deepEqual(result.reviews.map((r: {id: number; state: string; createdAt: string})=>[r.id,r.state,r.createdAt]),[[1,'COMMENTED',submitted],[2,'APPROVED',submitted]]);
  assert.ok(!JSON.stringify(result.reviews).includes('private draft'));
  assert.equal(result.commits[0].message,'First line');assert.equal(result.commits[0].createdAt,submitted);
  assert.ok(calls.some(path=>path.includes('/reviews')&&path.includes('page=2')));
});

test('oversized review summaries are rejected before any mutation', async()=>{
  let mutations=0;
  await assert.rejects(executeGithub({operation:'submit',url,expectedHead:'head',expectedBase:'merge-base',event:'APPROVE',body:'x'.repeat(60001)},'token',fake((path,init)=>{
    if(path.endsWith('/graphql'))return {data:{viewer:{login:'reviewer'},repository:{pullRequest:{reviews:{nodes:[]}}}}};
    if(init.method==='POST')mutations++;
    return pr;
  })),/60,000/);
  assert.equal(mutations,0);
});

for(const event of ['COMMENT','APPROVE','REQUEST_CHANGES']) for(const pending of [false,true]) {
  test(`review submission sends exactly one ${event} write with pending=${pending}`,async()=>{
    const writes:unknown[]=[];
    const body=event==='APPROVE'?'':'Please cover the empty state.';
    await executeGithub({operation:'submit',url,expectedHead:'head',expectedBase:'merge-base',event,body,context:[{note:'PRIVATE_AI_NOTE'}]},'token',fake((path,init)=>{
      if(path.endsWith('/graphql')) {
        const payload=JSON.parse(String(init.body));
        if(payload.query.trimStart().startsWith('query'))return {data:{viewer:{login:'reviewer'},repository:{pullRequest:{reviews:{nodes:pending?[{id:'OWN',author:{login:'reviewer'},commit:{oid:'head'}},{id:'OTHER',author:{login:'someone'},commit:{oid:'head'}}]:[]}}}}};
        writes.push(payload.variables.input);
        return {data:{submitPullRequestReview:{pullRequestReview:{id:'OWN',state:event}}}};
      }
      if(init.method==='POST')writes.push(JSON.parse(String(init.body)));
      return pr;
    }));
    assert.deepEqual(writes,[pending?{pullRequestReviewId:'OWN',event,...(body?{body}:{})}:{commit_id:'head',body,event}]);
    assert.ok(!JSON.stringify(writes).includes('PRIVATE_AI_NOTE'));
  });
}

test('a commit arriving after review lookup blocks publication',async()=>{
  let snapshots=0,writes=0;
  await assert.rejects(executeGithub({operation:'submit',url,expectedHead:'head',expectedBase:'merge-base',event:'APPROVE'},'token',fake((path,init)=>{
    if(path.endsWith('/graphql')) {
      const payload=JSON.parse(String(init.body));
      if(payload.query.trimStart().startsWith('mutation'))writes++;
      return {data:{viewer:{login:'reviewer'},repository:{pullRequest:{reviews:{nodes:[{id:'OWN',author:{login:'reviewer'},commit:{oid:'head'}}]}}}}};
    }
    snapshots++;return snapshots>=3?{...pr,head:{...pr.head,sha:'new-head'}}:pr;
  })),/New commits/);
  assert.equal(writes,0);
});

test('existing pending line comments can be submitted with no summary',async()=>{
  let sent:unknown;
  await executeGithub({operation:'submit',url,expectedHead:'head',expectedBase:'merge-base',event:'COMMENT',body:''},'token',fake((path,init)=>{
    if(!path.endsWith('/graphql'))return pr;
    const payload=JSON.parse(String(init.body));
    if(payload.query.trimStart().startsWith('query'))return {data:{viewer:{login:'reviewer'},repository:{pullRequest:{reviews:{nodes:[{id:'OWN',author:{login:'reviewer'},commit:{oid:'head'}}]}}}}};
    sent=payload.variables.input;return {data:{submitPullRequestReview:{pullRequestReview:{id:'OWN',state:'COMMENTED'}}}};
  }));
  assert.deepEqual(sent,{pullRequestReviewId:'OWN',event:'COMMENT'});
});
