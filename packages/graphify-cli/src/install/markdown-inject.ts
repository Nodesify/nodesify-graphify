import * as fs from 'fs';
import * as path from 'path';

const SECTION_HEADER = '## graphify';

export function injectSection(filePath: string, content: string): boolean {
  const dir = path.dirname(filePath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  let existing = '';
  if (fs.existsSync(filePath)) {
    existing = fs.readFileSync(filePath, 'utf-8');
  }

  if (existing.includes(SECTION_HEADER)) {
    return false;
  }

  const section = '\n' + content + '\n';
  fs.writeFileSync(filePath, existing + section, 'utf-8');
  return true;
}

export function removeSection(filePath: string): boolean {
  if (!fs.existsSync(filePath)) {
    return false;
  }

  let content = fs.readFileSync(filePath, 'utf-8');
  const regex = new RegExp(
    '\\n*' + SECTION_HEADER.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '\\n.*?(?=\\n## |$)',
    'gs'
  );
  const updated = content.replace(regex, '');

  if (updated.trim().length === 0) {
    fs.unlinkSync(filePath);
  } else {
    fs.writeFileSync(filePath, updated, 'utf-8');
  }
  return updated !== content;
}

export const PROJECT_MD_SECTION = `## graphify

This project has a nodesify-graphify knowledge graph at .graphify/.

Rules:
- MUST read .graphify/graph_report.md before searching files for architecture or codebase questions
- MUST use \`nodesify-graphify query "<question>"\`, \`nodesify-graphify path "<A>" "<B>"\`, or \`nodesify-graphify explain "<concept>"\` for cross-module questions — do NOT grep/read files directly for these
- After modifying code files in this session, run \`nodesify-graphify update .\` to keep the graph current`;

export const SKILL_REGISTRATION = `
# graphify
- **graphify** (\`~/.claude/skills/graphify/SKILL.md\`) - any input to knowledge graph. Trigger: \`/graphify\`
When the user types \`/graphify\`, invoke the Skill tool with \`skill: "graphify"\` before doing anything else.
`;
