const { execSync } = require('child_process');

const mode = process.env.AIDI_ENV || 'production';
console.log(`Building with mode: ${mode}`);

// 运行类型检查和构建
const cmd = process.platform === 'win32'
  ? `npx vue-tsc --noEmit && npx vite build --mode ${mode}`
  : `npx vue-tsc --noEmit && npx vite build --mode ${mode}`;

execSync(cmd, { stdio: 'inherit' });
