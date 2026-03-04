import { execSync } from 'child_process';

const mode = process.env.AIDI_ENV || 'production';
console.log(`[build.js] AIDI_ENV=${process.env.AIDI_ENV}`);
console.log(`[build.js] Building with mode: ${mode}`);

const cmd = `npx vue-tsc --noEmit && npx vite build --mode ${mode}`;

execSync(cmd, {
  stdio: 'inherit',
  env: { ...process.env, AIDI_ENV: mode }
});
