export interface PlatformConfig {
    skillFile: string;
    skillDst: string;
    claudeMd: boolean;
    agentsMd: boolean;
    geminiMd: boolean;
    settingsHook: 'claude' | 'codex' | 'gemini' | 'opencode' | 'none';
}
export declare const PLATFORMS: Record<string, PlatformConfig>;
export declare const PLATFORM_NAMES: string[];
//# sourceMappingURL=platforms.d.ts.map