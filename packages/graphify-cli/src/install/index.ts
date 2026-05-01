import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import { PLATFORMS, PlatformConfig } from './platforms';
import { injectSection, removeSection, PROJECT_MD_SECTION, SKILL_REGISTRATION } from './markdown-inject';
import {
  injectClaudeHook, removeClaudeHook,
  injectCodexHook, removeCodexHook,
  injectGeminiHook, removeGeminiHook,
  injectOpenCodePlugin, removeOpenCodePlugin,
  injectCursorRule, removeCursorRule,
  injectKiroSteering, removeKiroSteering,
} from './settings-inject';

function getSkillDir(): string {
  return path.resolve(__dirname, '..', '..', 'skills');
}

function copySkillFile(platform: string, cfg: PlatformConfig): string[] {
  const messages: string[] = [];
  if (!cfg.skillFile) return messages;

  const src = path.join(getSkillDir(), cfg.skillFile);
  if (!fs.existsSync(src)) {
    messages.push(`Skill file not found: ${src}`);
    return messages;
  }

  const homeDir = os.homedir();
  const dst = path.join(homeDir, cfg.skillDst);

  if (platform === 'claude') {
    const configDir = process.env.CLAUDE_CONFIG_DIR;
    if (configDir) {
      const overrideDst = path.join(configDir, 'skills', 'graphify', 'SKILL.md');
      copyFile(src, overrideDst);
      messages.push(`Skill file -> ${overrideDst}`);
      writeVersionStamp(path.dirname(overrideDst));
      return messages;
    }
  }

  copyFile(src, dst);
  messages.push(`Skill file -> ${dst}`);
  writeVersionStamp(path.dirname(dst));
  return messages;
}

function copyFile(src: string, dst: string) {
  const dir = path.dirname(dst);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  fs.copyFileSync(src, dst);
}

function writeVersionStamp(dir: string) {
  const stampPath = path.join(dir, '.graphify_version');
  try { fs.writeFileSync(stampPath, require('../../package.json').version + '\n', 'utf-8'); } catch { /* ignore */ }
}

export function installPlatform(platform: string, projectDir: string): string[] {
  const messages: string[] = [];

  if (platform === 'cursor') {
    if (injectCursorRule(projectDir)) {
      messages.push('Cursor rule -> .cursor/rules/graphify.mdc');
    } else {
      messages.push('Cursor rule: already installed');
    }
    return messages;
  }

  if (platform === 'kiro') {
    const cfg = PLATFORMS.kiro;
    messages.push(...copySkillFile('kiro', cfg));
    if (injectKiroSteering(projectDir)) {
      messages.push('Kiro steering -> .kiro/steering/graphify.md');
    } else {
      messages.push('Kiro steering: already installed');
    }
    return messages;
  }

  const cfg = PLATFORMS[platform];
  if (!cfg) {
    messages.push(`Unknown platform: ${platform}. Available: ${Object.keys(PLATFORMS).join(', ')}`);
    return messages;
  }

  messages.push(...copySkillFile(platform, cfg));

  if (cfg.claudeMd) {
    const claudeMdPath = path.join(os.homedir(), '.claude', 'CLAUDE.md');
    if (injectSection(claudeMdPath, SKILL_REGISTRATION)) {
      messages.push('User CLAUDE.md: skill registration added');
    } else {
      messages.push('User CLAUDE.md: already registered');
    }
  }

  const projectMd = path.join(projectDir, 'CLAUDE.md');
  if (cfg.claudeMd || cfg.agentsMd || cfg.geminiMd) {
    if (cfg.claudeMd && injectSection(projectMd, PROJECT_MD_SECTION)) {
      messages.push('Project CLAUDE.md: graphify section added');
    } else if (cfg.claudeMd) {
      messages.push('Project CLAUDE.md: already has graphify section');
    }

    if (cfg.agentsMd) {
      const agentsMd = path.join(projectDir, 'AGENTS.md');
      if (injectSection(agentsMd, PROJECT_MD_SECTION)) {
        messages.push('Project AGENTS.md: graphify section added');
      } else {
        messages.push('Project AGENTS.md: already has graphify section');
      }
    }

    if (cfg.geminiMd) {
      const geminiMd = path.join(projectDir, 'GEMINI.md');
      if (injectSection(geminiMd, PROJECT_MD_SECTION)) {
        messages.push('Project GEMINI.md: graphify section added');
      } else {
        messages.push('Project GEMINI.md: already has graphify section');
      }
    }
  }

  switch (cfg.settingsHook) {
    case 'claude':
      if (injectClaudeHook(projectDir)) {
        messages.push('Claude PreToolUse hook -> .claude/settings.json');
      } else {
        messages.push('Claude PreToolUse hook: already installed');
      }
      break;
    case 'codex':
      if (injectCodexHook(projectDir)) {
        messages.push('Codex PreToolUse hook -> .codex/hooks.json');
      } else {
        messages.push('Codex PreToolUse hook: already installed');
      }
      break;
    case 'gemini':
      if (injectGeminiHook(projectDir)) {
        messages.push('Gemini BeforeTool hook -> .gemini/settings.json');
      } else {
        messages.push('Gemini BeforeTool hook: already installed');
      }
      break;
    case 'opencode':
      if (injectOpenCodePlugin(projectDir)) {
        messages.push('OpenCode plugin -> .opencode/plugins/graphify.js');
      } else {
        messages.push('OpenCode plugin: already installed');
      }
      break;
  }

  return messages;
}

export function uninstallPlatform(platform: string, projectDir: string): string[] {
  const messages: string[] = [];

  if (platform === 'cursor') {
    if (removeCursorRule(projectDir)) {
      messages.push('Cursor rule: removed');
    } else {
      messages.push('Cursor rule: not found');
    }
    return messages;
  }

  if (platform === 'kiro') {
    const cfg = PLATFORMS.kiro;
    if (cfg.skillFile) {
      const homeDir = os.homedir();
      const dst = path.join(homeDir, cfg.skillDst);
      try { fs.unlinkSync(dst); messages.push(`Skill file removed: ${dst}`); } catch { messages.push('Skill file: not found'); }
    }
    if (removeKiroSteering(projectDir)) {
      messages.push('Kiro steering: removed');
    } else {
      messages.push('Kiro steering: not found');
    }
    return messages;
  }

  const cfg = PLATFORMS[platform];
  if (!cfg) {
    messages.push(`Unknown platform: ${platform}`);
    return messages;
  }

  if (cfg.skillFile) {
    const homeDir = os.homedir();
    let dst = path.join(homeDir, cfg.skillDst);
    if (platform === 'claude') {
      const configDir = process.env.CLAUDE_CONFIG_DIR;
      if (configDir) dst = path.join(configDir, 'skills', 'graphify', 'SKILL.md');
    }
    try { fs.unlinkSync(dst); messages.push(`Skill file removed: ${dst}`); } catch { messages.push('Skill file: not found'); }
  }

  if (cfg.claudeMd) {
    const claudeMdPath = path.join(os.homedir(), '.claude', 'CLAUDE.md');
    removeSection(claudeMdPath);
    messages.push('User CLAUDE.md: graphify section removed');
  }

  if (cfg.claudeMd) {
    removeSection(path.join(projectDir, 'CLAUDE.md'));
    messages.push('Project CLAUDE.md: graphify section removed');
  }
  if (cfg.agentsMd) {
    removeSection(path.join(projectDir, 'AGENTS.md'));
    messages.push('Project AGENTS.md: graphify section removed');
  }
  if (cfg.geminiMd) {
    removeSection(path.join(projectDir, 'GEMINI.md'));
    messages.push('Project GEMINI.md: graphify section removed');
  }

  switch (cfg.settingsHook) {
    case 'claude':
      removeClaudeHook(projectDir);
      messages.push('Claude PreToolUse hook: removed');
      break;
    case 'codex':
      removeCodexHook(projectDir);
      messages.push('Codex PreToolUse hook: removed');
      break;
    case 'gemini':
      removeGeminiHook(projectDir);
      messages.push('Gemini BeforeTool hook: removed');
      break;
    case 'opencode':
      removeOpenCodePlugin(projectDir);
      messages.push('OpenCode plugin: removed');
      break;
  }

  return messages;
}
