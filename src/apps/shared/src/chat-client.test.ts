import { describe, it, expect } from "vitest";

import { normalizeSkillOptions } from "./chat-client";

describe("normalizeSkillOptions", () => {
  it("parses skills grouped in `skills` buckets", () => {
    const raw = {
      skills: [
        {
          cwd: "/repo",
          skills: [{ name: "todo", path: "/repo/.homie/skills/todo", description: "todo helper" }],
          errors: [],
        },
      ],
    };

    expect(normalizeSkillOptions(raw)).toEqual([
      {
        name: "todo",
        path: "/repo/.homie/skills/todo",
        description: "todo helper",
      },
    ]);
  });

  it("parses skills from `data` envelopes", () => {
    const raw = {
      data: [
        {
          cwd: "/repo",
          skills: [{ name: "build", path: "/repo/.homie/skills/build", description: "run builds" }],
          errors: [],
        },
      ],
    };

    expect(normalizeSkillOptions(raw)).toEqual([
      {
        name: "build",
        path: "/repo/.homie/skills/build",
        description: "run builds",
      },
    ]);
  });

  it("parses direct skill arrays", () => {
    const raw = {
      data: [{ name: "local", path: "/home/me/.homie/skills/local" }],
    };

    expect(normalizeSkillOptions(raw)).toEqual([
      {
        name: "local",
        path: "/home/me/.homie/skills/local",
      },
    ]);
  });

  it("deduplicates duplicated payload shapes", () => {
    const raw = {
      skills: [
        { name: "deploy", path: "/repo/.homie/skills/deploy" },
        { name: "deploy", path: "/repo/.homie/skills/deploy" },
      ],
    };

    expect(normalizeSkillOptions(raw)).toEqual([
      {
        name: "deploy",
        path: "/repo/.homie/skills/deploy",
      },
    ]);
  });

  it("parses raw arrays of skill entries", () => {
    expect(
      normalizeSkillOptions([
        { name: "inline", path: "/repo/.homie/skills/inline" },
      ]),
    ).toEqual([{ name: "inline", path: "/repo/.homie/skills/inline" }]);
  });
});
