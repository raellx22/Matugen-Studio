import test from 'node:test';
import assert from 'node:assert/strict';
import { resolve, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

const root = resolve(fileURLToPath(new URL('..', import.meta.url)));
const sourceRoot = resolve(root, 'src');
const config = ts.readConfigFile(resolve(root, 'tsconfig.json'), ts.sys.readFile);
if (config.error) throw new Error(ts.flattenDiagnosticMessageText(config.error.messageText, '\n'));
const project = ts.parseJsonConfigFileContent(config.config, ts.sys, root);
const dialogs = new Set(['confirm', 'alert', 'prompt']);

function browserDialogCalls(program, files = program.getSourceFiles().filter(file => file.fileName.startsWith(sourceRoot + sep))) {
  const checker = program.getTypeChecker();
  const isDom = declaration => declaration.getSourceFile().fileName.endsWith('/lib.dom.d.ts');
  const isBrowserGlobal = expression => {
    if (!ts.isIdentifier(expression) || !['window', 'self', 'globalThis'].includes(expression.text)) return false;
    const symbol = checker.getSymbolAtLocation(expression);
    return !!symbol && (symbol.declarations?.some(isDom) || (expression.text === 'globalThis' && !symbol.declarations?.length));
  };
  const isBrowserDialog = (expression, seen = new Set()) => {
    if (ts.isIdentifier(expression)) {
      const symbol = checker.getSymbolAtLocation(expression);
      if (!symbol) return false;
      if (dialogs.has(expression.text) && symbol.declarations?.some(isDom)) return true;
      if (seen.has(symbol)) return false;
      seen.add(symbol);
      const declaration = symbol.valueDeclaration;
      return !!declaration && ts.isVariableDeclaration(declaration) && !!declaration.initializer &&
        isBrowserDialog(declaration.initializer, seen);
    }
    if (ts.isPropertyAccessExpression(expression) || ts.isElementAccessExpression(expression)) {
      const name = ts.isPropertyAccessExpression(expression) ? expression.name.text :
        ts.isStringLiteral(expression.argumentExpression) ? expression.argumentExpression.text : '';
      return dialogs.has(name) && isBrowserGlobal(expression.expression);
    }
    return false;
  };
  const failures = [];
  for (const file of files) {
    const visit = node => {
      if (ts.isCallExpression(node) && isBrowserDialog(node.expression)) {
        const line = file.getLineAndCharacterOfPosition(node.getStart(file)).line + 1;
        failures.push(`${relative(root, file.fileName)}:${line}: ${node.expression.getText(file)}`);
      }
      ts.forEachChild(node, visit);
    };
    visit(file);
  }
  return failures;
}

test('dialog audit distinguishes browser calls from local and Tauri helpers', () => {
  const filename = resolve(sourceRoot, '__dialog_audit_fixture__.ts');
  const text = `
    import { confirm } from '@tauri-apps/plugin-dialog';
    confirm('Tauri');
    function alert() {} alert();
    const local = { confirm() {} }; local.confirm();
    window.confirm('browser'); globalThis.alert('browser'); self['prompt']('browser');
    const browserAlias = window.confirm; browserAlias('browser');
  `;
  const host = ts.createCompilerHost(project.options);
  const getSourceFile = host.getSourceFile.bind(host);
  host.getSourceFile = (file, languageVersion, onError, shouldCreateNewSourceFile) =>
    file === filename ? ts.createSourceFile(file, text, languageVersion, true) :
      getSourceFile(file, languageVersion, onError, shouldCreateNewSourceFile);
  const program = ts.createProgram([...project.fileNames, filename], project.options, host);
  const file = program.getSourceFile(filename);
  assert.ok(file);
  assert.deepEqual(browserDialogCalls(program, [file]).map(call => call.split(': ').at(-1)),
    ['window.confirm', 'globalThis.alert', "self['prompt']", 'browserAlias']);
});

test('frontend never calls browser-native dialogs', () => {
  const program = ts.createProgram(project.fileNames, project.options);
  assert.deepEqual(browserDialogCalls(program), [], 'Use @tauri-apps/plugin-dialog instead of browser dialogs');
});
