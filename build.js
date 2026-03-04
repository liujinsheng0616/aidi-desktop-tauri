import { execSync } from 'child_process';

const mode = process.env.AIDI_ENV || 'production';
console.log(`Building with mode: ${mode}`);

const cmd = `npx vue-tsc --noEmit && npx vite build --mode ${mode}`;

execSync(cmd, { stdio: 'inherit' });
