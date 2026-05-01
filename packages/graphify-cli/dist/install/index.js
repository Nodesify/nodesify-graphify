"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.installPlatform = installPlatform;
exports.uninstallPlatform = uninstallPlatform;
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const os = __importStar(require("os"));
const platforms_1 = require("./platforms");
const markdown_inject_1 = require("./markdown-inject");
const settings_inject_1 = require("./settings-inject");
function getSkillDir() {
    return path.resolve(__dirname, '..', '..', 'skills');
}
function copySkillFile(platform, cfg) {
    const messages = [];
    if (!cfg.skillFile)
        return messages;
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
function copyFile(src, dst) {
    const dir = path.dirname(dst);
    if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
    }
    fs.copyFileSync(src, dst);
}
function writeVersionStamp(dir) {
    const stampPath = path.join(dir, '.graphify_version');
    try {
        fs.writeFileSync(stampPath, require('../../package.json').version + '\n', 'utf-8');
    }
    catch { /* ignore */ }
}
function installPlatform(platform, projectDir) {
    const messages = [];
    if (platform === 'cursor') {
        if ((0, settings_inject_1.injectCursorRule)(projectDir)) {
            messages.push('Cursor rule -> .cursor/rules/graphify.mdc');
        }
        else {
            messages.push('Cursor rule: already installed');
        }
        return messages;
    }
    if (platform === 'kiro') {
        const cfg = platforms_1.PLATFORMS.kiro;
        messages.push(...copySkillFile('kiro', cfg));
        if ((0, settings_inject_1.injectKiroSteering)(projectDir)) {
            messages.push('Kiro steering -> .kiro/steering/graphify.md');
        }
        else {
            messages.push('Kiro steering: already installed');
        }
        return messages;
    }
    const cfg = platforms_1.PLATFORMS[platform];
    if (!cfg) {
        messages.push(`Unknown platform: ${platform}. Available: ${Object.keys(platforms_1.PLATFORMS).join(', ')}`);
        return messages;
    }
    messages.push(...copySkillFile(platform, cfg));
    if (cfg.claudeMd) {
        const claudeMdPath = path.join(os.homedir(), '.claude', 'CLAUDE.md');
        if ((0, markdown_inject_1.injectSection)(claudeMdPath, markdown_inject_1.SKILL_REGISTRATION)) {
            messages.push('User CLAUDE.md: skill registration added');
        }
        else {
            messages.push('User CLAUDE.md: already registered');
        }
    }
    const projectMd = path.join(projectDir, 'CLAUDE.md');
    if (cfg.claudeMd || cfg.agentsMd || cfg.geminiMd) {
        if (cfg.claudeMd && (0, markdown_inject_1.injectSection)(projectMd, markdown_inject_1.PROJECT_MD_SECTION)) {
            messages.push('Project CLAUDE.md: graphify section added');
        }
        else if (cfg.claudeMd) {
            messages.push('Project CLAUDE.md: already has graphify section');
        }
        if (cfg.agentsMd) {
            const agentsMd = path.join(projectDir, 'AGENTS.md');
            if ((0, markdown_inject_1.injectSection)(agentsMd, markdown_inject_1.PROJECT_MD_SECTION)) {
                messages.push('Project AGENTS.md: graphify section added');
            }
            else {
                messages.push('Project AGENTS.md: already has graphify section');
            }
        }
        if (cfg.geminiMd) {
            const geminiMd = path.join(projectDir, 'GEMINI.md');
            if ((0, markdown_inject_1.injectSection)(geminiMd, markdown_inject_1.PROJECT_MD_SECTION)) {
                messages.push('Project GEMINI.md: graphify section added');
            }
            else {
                messages.push('Project GEMINI.md: already has graphify section');
            }
        }
    }
    switch (cfg.settingsHook) {
        case 'claude':
            if ((0, settings_inject_1.injectClaudeHook)(projectDir)) {
                messages.push('Claude PreToolUse hook -> .claude/settings.json');
            }
            else {
                messages.push('Claude PreToolUse hook: already installed');
            }
            break;
        case 'codex':
            if ((0, settings_inject_1.injectCodexHook)(projectDir)) {
                messages.push('Codex PreToolUse hook -> .codex/hooks.json');
            }
            else {
                messages.push('Codex PreToolUse hook: already installed');
            }
            break;
        case 'gemini':
            if ((0, settings_inject_1.injectGeminiHook)(projectDir)) {
                messages.push('Gemini BeforeTool hook -> .gemini/settings.json');
            }
            else {
                messages.push('Gemini BeforeTool hook: already installed');
            }
            break;
        case 'opencode':
            if ((0, settings_inject_1.injectOpenCodePlugin)(projectDir)) {
                messages.push('OpenCode plugin -> .opencode/plugins/graphify.js');
            }
            else {
                messages.push('OpenCode plugin: already installed');
            }
            break;
    }
    return messages;
}
function uninstallPlatform(platform, projectDir) {
    const messages = [];
    if (platform === 'cursor') {
        if ((0, settings_inject_1.removeCursorRule)(projectDir)) {
            messages.push('Cursor rule: removed');
        }
        else {
            messages.push('Cursor rule: not found');
        }
        return messages;
    }
    if (platform === 'kiro') {
        const cfg = platforms_1.PLATFORMS.kiro;
        if (cfg.skillFile) {
            const homeDir = os.homedir();
            const dst = path.join(homeDir, cfg.skillDst);
            try {
                fs.unlinkSync(dst);
                messages.push(`Skill file removed: ${dst}`);
            }
            catch {
                messages.push('Skill file: not found');
            }
        }
        if ((0, settings_inject_1.removeKiroSteering)(projectDir)) {
            messages.push('Kiro steering: removed');
        }
        else {
            messages.push('Kiro steering: not found');
        }
        return messages;
    }
    const cfg = platforms_1.PLATFORMS[platform];
    if (!cfg) {
        messages.push(`Unknown platform: ${platform}`);
        return messages;
    }
    if (cfg.skillFile) {
        const homeDir = os.homedir();
        let dst = path.join(homeDir, cfg.skillDst);
        if (platform === 'claude') {
            const configDir = process.env.CLAUDE_CONFIG_DIR;
            if (configDir)
                dst = path.join(configDir, 'skills', 'graphify', 'SKILL.md');
        }
        try {
            fs.unlinkSync(dst);
            messages.push(`Skill file removed: ${dst}`);
        }
        catch {
            messages.push('Skill file: not found');
        }
    }
    if (cfg.claudeMd) {
        const claudeMdPath = path.join(os.homedir(), '.claude', 'CLAUDE.md');
        (0, markdown_inject_1.removeSection)(claudeMdPath);
        messages.push('User CLAUDE.md: graphify section removed');
    }
    if (cfg.claudeMd) {
        (0, markdown_inject_1.removeSection)(path.join(projectDir, 'CLAUDE.md'));
        messages.push('Project CLAUDE.md: graphify section removed');
    }
    if (cfg.agentsMd) {
        (0, markdown_inject_1.removeSection)(path.join(projectDir, 'AGENTS.md'));
        messages.push('Project AGENTS.md: graphify section removed');
    }
    if (cfg.geminiMd) {
        (0, markdown_inject_1.removeSection)(path.join(projectDir, 'GEMINI.md'));
        messages.push('Project GEMINI.md: graphify section removed');
    }
    switch (cfg.settingsHook) {
        case 'claude':
            (0, settings_inject_1.removeClaudeHook)(projectDir);
            messages.push('Claude PreToolUse hook: removed');
            break;
        case 'codex':
            (0, settings_inject_1.removeCodexHook)(projectDir);
            messages.push('Codex PreToolUse hook: removed');
            break;
        case 'gemini':
            (0, settings_inject_1.removeGeminiHook)(projectDir);
            messages.push('Gemini BeforeTool hook: removed');
            break;
        case 'opencode':
            (0, settings_inject_1.removeOpenCodePlugin)(projectDir);
            messages.push('OpenCode plugin: removed');
            break;
    }
    return messages;
}
//# sourceMappingURL=index.js.map