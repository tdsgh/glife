import { expect, test } from "vitest";
import { fragmentShader, vertexShader } from "./gridShader";

test("the vertex shader holds main() only; helpers belong in the fragment shader", () => {
  const functions = [...vertexShader.matchAll(/^\s*\w+\s+(\w+)\s*\(/gm)].map((m) => m[1]);
  expect(functions).toEqual(["main"]);
});

test("the fragment shader holds the grid mapping helpers", () => {
  for (const name of ["cellAt", "cellCenter", "edgeDist"]) expect(fragmentShader).toMatch(new RegExp(`\\b${name}\\s*\\(`));
  expect(fragmentShader).toContain("uniform int uTiling");
});
