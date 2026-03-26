import { execSync } from 'child_process';
import { readFileSync, existsSync, writeFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const mode = process.env.AIDI_ENV || 'production';
console.log(`[build.js] AIDI_ENV=${process.env.AIDI_ENV}`);
console.log(`[build.js] Building with mode: ${mode}`);

// 映射环境名称到配置文件
const envFileMap = {
  'prod': '.env.production',
  'test': '.env.test',
  'development': '.env.development'
};
const envFile = envFileMap[mode] || `.env.${mode}`;
const envPath = resolve(__dirname, envFile);

if (!existsSync(envPath)) {
  console.error(`[build.js] 错误: ${envFile} 文件不存在！`);
  process.exit(1);
}

console.log(`[build.js] 读取环境配置: ${envFile}`);
const envContent = readFileSync(envPath, 'utf-8');
console.log(envContent);

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
  console.error(`[build.js] 错误: VITE_APP_DOMAIN 未在 ${envFile} 中定义！`);
  process.exit(1);
}
console.log(`[build.js] APP_DOMAIN: ${appDomain}`);

// 替换 tauri.conf.json 中的占位符
const tauriConfPath = resolve(__dirname, 'src-tauri/tauri.conf.json');
let tauriConf = readFileSync(tauriConfPath, 'utf-8');

if (tauriConf.includes('{{APP_DOMAIN}}')) {
  tauriConf = tauriConf.replace(/\{\{APP_DOMAIN\}\}/g, appDomain);
  writeFileSync(tauriConfPath, tauriConf);
  console.log(`[build.js] 已替换 tauri.conf.json 中的 APP_DOMAIN 占位符`);
} else {
  console.log(`[build.js] tauri.conf.json 中未找到 APP_DOMAIN 占位符，跳过替换`);
}

// 执行构建命令（Vite 的 --mode 决定加载哪个 .env 文件，prod 对应 .env.production）
const viteModeMap = { 'prod': 'production', 'test': 'test', 'development': 'development' };
const viteMode = viteModeMap[mode] || mode;
const cmd = `npx vue-tsc --noEmit && npx vite build --mode ${viteMode}`;
console.log(`[build.js] 执行命令: ${cmd}`);

execSync(cmd, {
  stdio: 'inherit',
  env: { ...process.env, AIDI_ENV: mode }
});
