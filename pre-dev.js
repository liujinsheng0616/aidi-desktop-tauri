/**
 * 开发前脚本：替换 tauri.conf.json 中的占位符
 * 运行方式: node pre-dev.js [mode]
 * 默认使用 .env.test 配置
 */
import { readFileSync, existsSync, writeFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const mode = process.argv[2] || 'test';
console.log(`[pre-dev.js] 使用环境: ${mode}`);

// 读取环境变量文件
const envFile = `.env.${mode}`;
const envPath = resolve(__dirname, envFile);

if (!existsSync(envPath)) {
  console.error(`[pre-dev.js] 错误: ${envFile} 文件不存在！`);
  process.exit(1);
}

const envContent = readFileSync(envPath, 'utf-8');

// 解析环境变量
const envVars = {};
envContent.split('\n').forEach(line => {
  const trimmed = line.trim();
  if (trimmed && !trimmed.startsWith('#')) {
    const [key, ...valueParts] = trimmed.split('=');
    if (key && valueParts.length > 0) {
      envVars[key.trim()] = valueParts.join('=').trim();
    }
  }
});

const appDomain = envVars.VITE_APP_DOMAIN;
if (!appDomain) {
  console.error(`[pre-dev.js] 错误: VITE_APP_DOMAIN 未在 ${envFile} 中定义！`);
  process.exit(1);
}
console.log(`[pre-dev.js] APP_DOMAIN: ${appDomain}`);

// 替换 tauri.conf.json 中的占位符
const tauriConfPath = resolve(__dirname, 'src-tauri/tauri.conf.json');
let tauriConf = readFileSync(tauriConfPath, 'utf-8');

if (tauriConf.includes('{{APP_DOMAIN}}')) {
  tauriConf = tauriConf.replace(/\{\{APP_DOMAIN\}\}/g, appDomain);
  writeFileSync(tauriConfPath, tauriConf);
  console.log(`[pre-dev.js] 已替换 tauri.conf.json 中的 APP_DOMAIN 占位符为 ${appDomain}`);
} else {
  console.log(`[pre-dev.js] tauri.conf.json 中未找到占位符，跳过替换`);
}
