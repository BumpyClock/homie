import { defineConfig } from 'vitest/config';
import path from 'node:path';

export default defineConfig({
  resolve: {
    alias: {
      '@/': path.resolve(__dirname, './') + '/',
      '@react-native-async-storage/async-storage':
        path.resolve(__dirname, '__tests__/helpers/mock-async-storage.ts'),
      'react-native': path.resolve(__dirname, '__tests__/helpers/mock-react-native.ts'),
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    include: ['__tests__/**/*.test.ts'],
    setupFiles: ['__tests__/setup.ts'],
  },
});
