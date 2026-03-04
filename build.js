import { execSync } from 'child_process';
import { readFileSync, existsSync } from 'fs';

const mode = process.env.AIDI_ENV || 'production';
console.log(`[build.js] AIDI_ENV=${process.env.AIDI_ENV}`);
console.log(`[build.js] Building with mode: ${mode}`);

// 检查 .env.test 文件内容
const envFile = `.env.${mode}`;
if (existsSync(envFile)) {
  console.log(`[build.js] ${envFile} 内容:`);
  console.log(readFileSync(envFile, 'utf-8'));
} else {
  console.log(`[build.js] 警告: ${envFile} 文件不存在！`);
}

const cmd = `npx vue-tsc --noEmit && npx vite build --mode ${mode}`;
console.log(`[build.js] 执行命令: ${cmd}`);

execSync(cmd, {
  stdio: 'inherit',
  env: { ...process.env, AIDI_ENV: mode }
});
