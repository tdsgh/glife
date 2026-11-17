import { expect, test } from "vitest";
import { vertexShader } from "./gridShader";

test("the vertex shader holds main() only; helpers belong in the fragment shader", () => {
  const functions = [...vertexShader.matchAll(/^\s*\w+\s+(\w+)\s*\(/gm)].map((m) => m[1]);
  expect(functions).toEqual(["main"]);
});
