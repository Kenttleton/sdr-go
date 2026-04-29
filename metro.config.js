const { getDefaultConfig } = require('expo/metro-config');
const path = require('path');

const appName = process.env.SDRGO_APP || 'analyzer';
const projectRoot = path.resolve(__dirname, 'apps', appName);
const workspaceRoot = __dirname;

const config = getDefaultConfig(projectRoot);

config.watchFolders = [workspaceRoot];
config.projectRoot = projectRoot;

config.resolver.nodeModulesPaths = [
    path.resolve(projectRoot, 'node_modules'),
    path.resolve(workspaceRoot, 'node_modules'),
];

module.exports = config;