import * as fs from 'fs';
import * as path from 'path';

const CONTEXT_MSG_RAW = 'nodesify-graphify: Knowledge graph exists. Read .graphify/graph_report.md for god nodes and community structure before searching raw files.';
const CONTEXT_MSG = CONTEXT_MSG_RAW.replace(/'/g, "\\'");

function readJson(filePath: string): any {
  if (!fs.existsSync(filePath)) return {};
  try {
    return JSON.parse(fs.readFileSync(filePath, 'utf-8'));
  } catch {
    return {};
  }
}

function writeJson(filePath: string, data: any) {
  const dir = path.dirname(filePath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + '\n', 'utf-8');
}

// ---- Claude Code (.claude/settings.json) ----

const CLAUDE_HOOK = {
  matcher: 'Bash',
  hooks: [{
    type: 'command',
    command: `node -e "try{var d=JSON.parse(require('fs').readFileSync(0,'utf8'));var c=(d.tool_input||d).command||'';if(/(grep|rg |ripgrep|find |fd |ack |ag )/.test(c)&&require('fs').existsSync('.graphify/graph.json')){process.stdout.write(JSON.stringify({hookSpecificOutput:{hookEventName:'PreToolUse',additionalContext:'${CONTEXT_MSG}'}}))}}catch(e){}"`,
  }],
};

export function injectClaudeHook(projectDir: string): boolean {
  const settingsPath = path.join(projectDir, '.claude', 'settings.json');
  const data = readJson(settingsPath);
  if (!data.hooks) data.hooks = {};
  if (!data.hooks.PreToolUse) data.hooks.PreToolUse = [];

  const existing = data.hooks.PreToolUse as any[];
  const alreadyExists = existing.some((h: any) =>
    h.matcher === 'Bash' && JSON.stringify(h.hooks).includes('graphify')
  );
  if (alreadyExists) return false;

  existing.push(CLAUDE_HOOK);
  writeJson(settingsPath, data);
  return true;
}

export function removeClaudeHook(projectDir: string): boolean {
  const settingsPath = path.join(projectDir, '.claude', 'settings.json');
  if (!fs.existsSync(settingsPath)) return false;

  const data = readJson(settingsPath);
  if (!data.hooks?.PreToolUse) return false;

  const before = data.hooks.PreToolUse.length;
  data.hooks.PreToolUse = (data.hooks.PreToolUse as any[]).filter((h: any) =>
    !(h.matcher === 'Bash' && JSON.stringify(h.hooks).includes('graphify'))
  );

  if (data.hooks.PreToolUse.length === 0) {
    delete data.hooks.PreToolUse;
  }
  if (Object.keys(data.hooks).length === 0) {
    delete data.hooks;
  }
  writeJson(settingsPath, data);
  return data.hooks?.PreToolUse?.length !== before;
}

// ---- Codex (.codex/hooks.json) ----

export function injectCodexHook(projectDir: string): boolean {
  const hooksPath = path.join(projectDir, '.codex', 'hooks.json');
  const data = readJson(hooksPath);
  if (!data.hooks) data.hooks = {};
  if (!data.hooks.PreToolUse) data.hooks.PreToolUse = [];

  const existing = data.hooks.PreToolUse as any[];
  const alreadyExists = existing.some((h: any) =>
    JSON.stringify(h.hooks || []).includes('graphify')
  );
  if (alreadyExists) return false;

  existing.push({
    matcher: 'Bash',
    hooks: [{
      type: 'command',
      command: `node -e "if(require('fs').existsSync('.graphify/graph.json')){process.stdout.write(JSON.stringify({hookSpecificOutput:{hookEventName:'PreToolUse',additionalContext:'${CONTEXT_MSG}'}}))}"`,
    }],
  });
  writeJson(hooksPath, data);
  return true;
}

export function removeCodexHook(projectDir: string): boolean {
  const hooksPath = path.join(projectDir, '.codex', 'hooks.json');
  if (!fs.existsSync(hooksPath)) return false;

  const data = readJson(hooksPath);
  if (!data.hooks?.PreToolUse) return false;

  const before = (data.hooks.PreToolUse as any[]).length;
  data.hooks.PreToolUse = (data.hooks.PreToolUse as any[]).filter((h: any) =>
    !JSON.stringify(h.hooks || []).includes('graphify')
  );
  writeJson(hooksPath, data);
  return (data.hooks.PreToolUse as any[]).length !== before;
}

// ---- Gemini (.gemini/settings.json) ----

export function injectGeminiHook(projectDir: string): boolean {
  const settingsPath = path.join(projectDir, '.gemini', 'settings.json');
  const data = readJson(settingsPath);
  if (!data.hooks) data.hooks = {};
  if (!data.hooks.BeforeTool) data.hooks.BeforeTool = [];

  const existing = data.hooks.BeforeTool as any[];
  const alreadyExists = existing.some((h: any) =>
    h.matcher === 'read_file|list_directory' && JSON.stringify(h.hooks || []).includes('graphify')
  );
  if (alreadyExists) return false;

  existing.push({
    matcher: 'read_file|list_directory',
    hooks: [{
      type: 'command',
      command: `node -e "var r={decision:'allow'};if(require('fs').existsSync('.graphify/graph.json')){r.additionalContext='${CONTEXT_MSG}'}process.stdout.write(JSON.stringify(r))"`,
    }],
  });
  writeJson(settingsPath, data);
  return true;
}

export function removeGeminiHook(projectDir: string): boolean {
  const settingsPath = path.join(projectDir, '.gemini', 'settings.json');
  if (!fs.existsSync(settingsPath)) return false;

  const data = readJson(settingsPath);
  if (!data.hooks?.BeforeTool) return false;

  const before = (data.hooks.BeforeTool as any[]).length;
  data.hooks.BeforeTool = (data.hooks.BeforeTool as any[]).filter((h: any) =>
    !JSON.stringify(h.hooks || []).includes('graphify')
  );
  writeJson(settingsPath, data);
  return (data.hooks.BeforeTool as any[]).length !== before;
}

// ---- OpenCode (.opencode/) ----

const OPENCODE_PLUGIN_JS = `// nodesify-graphify OpenCode plugin
import { existsSync } from "fs";
import { join } from "path";

export const GraphifyPlugin = async ({ directory }) => {
  let reminded = false;
  return {
    "tool.execute.before": async (input, output) => {
      if (reminded) return;
      if (!existsSync(join(directory, ".graphify", "graph.json"))) return;
      if (input.tool === "bash") {
        output.args.command =
          'echo "[nodesify-graphify] Knowledge graph available. Read .graphify/graph_report.md for architecture context." && ' +
          output.args.command;
        reminded = true;
      }
    },
  };
};
`;

export function injectOpenCodePlugin(projectDir: string): boolean {
  const pluginDir = path.join(projectDir, '.opencode', 'plugins');
  const pluginPath = path.join(pluginDir, 'graphify.js');
  if (fs.existsSync(pluginPath)) return false;

  if (!fs.existsSync(pluginDir)) {
    fs.mkdirSync(pluginDir, { recursive: true });
  }
  fs.writeFileSync(pluginPath, OPENCODE_PLUGIN_JS, 'utf-8');

  const configPath = path.join(projectDir, '.opencode', 'opencode.json');
  const config = readJson(configPath);
  if (!config.plugins) config.plugins = [];
  if (!config.plugins.includes('./plugins/graphify.js')) {
    config.plugins.push('./plugins/graphify.js');
  }
  writeJson(configPath, config);
  return true;
}

export function removeOpenCodePlugin(projectDir: string): boolean {
  const pluginPath = path.join(projectDir, '.opencode', 'plugins', 'graphify.js');
  if (!fs.existsSync(pluginPath)) return false;
  fs.unlinkSync(pluginPath);

  const configPath = path.join(projectDir, '.opencode', 'opencode.json');
  const config = readJson(configPath);
  if (config.plugins) {
    config.plugins = config.plugins.filter((p: string) => p !== './plugins/graphify.js');
    writeJson(configPath, config);
  }
  return true;
}

// ---- Cursor (.cursor/rules/graphify.mdc) ----

const CURSOR_RULE = `---
description: nodesify-graphify knowledge graph context
alwaysApply: true
---

This project has a nodesify-graphify knowledge graph at .graphify/.

- Before answering architecture or codebase questions, read .graphify/graph_report.md for god nodes and community structure
- For cross-module questions, use \`nodesify-graphify query\` and \`nodesify-graphify path\` instead of grep
- After modifying code files, run \`nodesify-graphify update .\` to keep the graph current
`;

export function injectCursorRule(projectDir: string): boolean {
  const ruleDir = path.join(projectDir, '.cursor', 'rules');
  const rulePath = path.join(ruleDir, 'graphify.mdc');
  if (fs.existsSync(rulePath)) return false;

  if (!fs.existsSync(ruleDir)) {
    fs.mkdirSync(ruleDir, { recursive: true });
  }
  fs.writeFileSync(rulePath, CURSOR_RULE, 'utf-8');
  return true;
}

export function removeCursorRule(projectDir: string): boolean {
  const rulePath = path.join(projectDir, '.cursor', 'rules', 'graphify.mdc');
  if (!fs.existsSync(rulePath)) return false;
  fs.unlinkSync(rulePath);
  return true;
}

// ---- Kiro (.kiro/steering/graphify.md) ----

const KIRO_STEERING = `---
inclusion: always
---

nodesify-graphify: A knowledge graph of this project lives in \`.graphify/\`. If \`.graphify/graph_report.md\` exists, read it before answering architecture questions, tracing dependencies, or searching files. Navigate by graph structure instead of grepping raw files.
`;

export function injectKiroSteering(projectDir: string): boolean {
  const steerDir = path.join(projectDir, '.kiro', 'steering');
  const steerPath = path.join(steerDir, 'graphify.md');
  if (fs.existsSync(steerPath)) return false;

  if (!fs.existsSync(steerDir)) {
    fs.mkdirSync(steerDir, { recursive: true });
  }
  fs.writeFileSync(steerPath, KIRO_STEERING, 'utf-8');
  return true;
}

export function removeKiroSteering(projectDir: string): boolean {
  const steerPath = path.join(projectDir, '.kiro', 'steering', 'graphify.md');
  if (!fs.existsSync(steerPath)) return false;
  fs.unlinkSync(steerPath);
  return true;
}
