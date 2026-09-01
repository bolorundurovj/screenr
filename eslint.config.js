// @ts-check
const eslint = require('@eslint/js');
const tseslint = require('typescript-eslint');
const angular = require('angular-eslint');
const prettier = require('eslint-config-prettier');

module.exports = tseslint.config(
    {
        ignores: ['dist/**', '.angular/**', 'src-tauri/**', 'node_modules/**'],
    },
    {
        files: ['**/*.ts'],
        extends: [
            eslint.configs.recommended,
            ...tseslint.configs.recommended,
            ...tseslint.configs.stylistic,
            ...angular.configs.tsRecommended,
            // Last, so formatting rules defer to Prettier.
            prettier,
        ],
        processor: angular.processInlineTemplates,
        rules: {
            '@angular-eslint/directive-selector': [
                'error',
                {type: 'attribute', prefix: 'app', style: 'camelCase'},
            ],
            '@angular-eslint/component-selector': [
                'error',
                {type: 'element', prefix: 'app', style: 'kebab-case'},
            ],
            // Tauri command rejections are unknown; narrowing every catch adds
            // noise without adding safety.
            '@typescript-eslint/no-explicit-any': 'warn',
            // ControlValueAccessor requires no-op defaults until Angular
            // registers the real callbacks.
            '@typescript-eslint/no-empty-function': ['error', {allow: ['arrowFunctions']}],
            '@typescript-eslint/no-unused-vars': [
                'error',
                {argsIgnorePattern: '^_', varsIgnorePattern: '^_'},
            ],
        },
    },
    {
        files: ['**/*.spec.ts'],
        rules: {
            // Stubs stand in for typed services; exactness is not the point.
            '@typescript-eslint/no-explicit-any': 'off',
        },
    },
    {
        files: ['**/*.html'],
        extends: [...angular.configs.templateRecommended, ...angular.configs.templateAccessibility],
        rules: {
            // Interactive rows use click handlers on buttons already; the rule
            // fires on the wrapper divs that exist purely for layout.
            '@angular-eslint/template/click-events-have-key-events': 'warn',
            '@angular-eslint/template/interactive-supports-focus': 'warn',
        },
    },
);
