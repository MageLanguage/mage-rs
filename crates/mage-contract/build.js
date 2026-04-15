import { YAML, file, write } from "bun";
import ejs from "ejs";

let definition = YAML.parse(await file("bytecode.yaml").text());
let bytecodeRenderer = ejs.compile(await file("template.ejs").text(), {});
let testRenderer = ejs.compile(await file("template_test.ejs").text(), {});

let splitWords = (value) => {
  return value
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .split(/[^a-zA-Z0-9]+/)
    .filter(Boolean);
};

let toUpperCamelCase = (value) => {
  return splitWords(value)
    .map((word) => word[0].toUpperCase() + word.slice(1).toLowerCase())
    .join("");
};

let toSnakeCase = (value) => {
  return splitWords(value)
    .map((word) => word.toLowerCase())
    .join("_");
};

class NameBuilder {
  constructor(instructionName) {
    this.base = toUpperCamelCase(instructionName);
    this.camel = toUpperCamelCase(instructionName);
    this.snake = toSnakeCase(instructionName);
  }

  extend(name) {
    this.camel += toUpperCamelCase(name);
    this.snake += "_" + toSnakeCase(name);
  }
}

let iterate = (instructions, callback) => {
  let index = 0;

  for (const [instructionName, instruction] of Object.entries(instructions)) {
    if (!instruction) {
      let nameBuilder = new NameBuilder(instructionName);
      callback(index, nameBuilder);
      index += 1;
    } else {
      if (instruction.type && instruction.data) {
        throw new Error(
          `instruction "${instructionName}": cannot have both "type" and "data"`
        );
      }

      if (!instruction.type && !instruction.data) {
        throw new Error(
          `instruction "${instructionName}": must have either "type" or "data"`
        );
      }

      let variants;

      if (instruction.type) {
        variants = definition.types[instruction.type];
        if (!variants) {
          throw new Error(
            `instruction "${instructionName}": unknown type "${instruction.type}"`
          );
        }
      } else {
        variants = instruction.data;
      }

      for (const variant of variants) {
        let nameBuilder = new NameBuilder(instructionName);
        let data = [];

        for (const [name, metadata] of Object.entries(variant)) {
          nameBuilder.extend(name);
          nameBuilder.extend(metadata[1]);

          data.push({
            name: toSnakeCase(name),
            type: metadata[0],
            kind: metadata[1],
          });
        }

        callback(index, nameBuilder, data);
        index += 1;
      }
    }
  }
};

let knownVariants = new Set();
let maximumOperands = 1;

iterate(definition.instructions, (_index, name, data) => {
  knownVariants.add(name.camel);
  if (Array.isArray(data)) {
    maximumOperands = Math.max(maximumOperands, data.length);
  }
});

function testValue(kind, fieldIndex) {
  if (kind === "offset") return fieldIndex * 8;
  return fieldIndex + 1;
}

function generateInstruction(name, data, indent) {
  if (!Array.isArray(data)) {
    return `${indent}Instruction::${name.camel}`;
  }
  let lines = [];
  lines.push(`${indent}Instruction::${name.camel}(${name.camel} {`);
  for (let fieldIndex = 0; fieldIndex < data.length; fieldIndex++) {
    lines.push(`${indent}    ${data[fieldIndex].name}: ${testValue(data[fieldIndex].kind, fieldIndex)},`);
  }
  lines.push(`${indent}})`);
  return lines.join("\n");
}

for (let index = 0; index < definition.superinstructions.length; index++) {
  for (const component of definition.superinstructions[index]) {
    if (!knownVariants.has(component)) {
      throw new Error(
        `superinstructions[${index}]: unknown instruction variant "${component}"`
      );
    }
  }
}

let context = {
  instructions: definition.instructions,
  superinstructions: definition.superinstructions,
  iterate,
  maximumOperands,
  generateInstruction,
};

await write("bytecode.rs", bytecodeRenderer(context));
await write("bytecode_test.rs", testRenderer(context));
