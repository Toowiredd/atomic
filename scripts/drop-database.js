// scripts/drop-database.js
import fs from 'fs';
import readline from 'readline';

// Database path - in development, the database is in the Tauri app data directory
function getDefaultDbPath() {
  const platform = process.platform;
  const home = process.env.HOME || process.env.USERPROFILE;

  if (platform === 'darwin') {
    return `${home}/Library/Application Support/com.atomic.app/atomic.db`;
  } else if (platform === 'linux') {
    return `${home}/.local/share/com.atomic.app/atomic.db`;
  } else if (platform === 'win32') {
    return `${process.env.APPDATA}/com.atomic.app/atomic.db`;
  }
  throw new Error(`Unsupported platform: ${platform}`);
}

// Prompt for confirmation
function promptConfirmation(message) {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  return new Promise((resolve) => {
    rl.question(message, (answer) => {
      rl.close();
      resolve(answer.toLowerCase() === 'yes');
    });
  });
}

// Create backup
function createBackup(dbPath) {
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-').split('T')[0];
  const backupPath = dbPath.replace('.db', `_backup_${timestamp}.db`);

  console.log(`\nCreating backup...`);
  fs.copyFileSync(dbPath, backupPath);
  console.log(`✓ Backup created: ${backupPath}\n`);

  return backupPath;
}

// Get file size in human readable format
function formatFileSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

// Main function
async function main() {
  const args = process.argv.slice(2);

  // Parse arguments
  let dbPath = null;
  let force = false;
  let backup = false;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--db' && args[i + 1]) {
      dbPath = args[i + 1];
      i++;
    } else if (args[i] === '--force') {
      force = true;
    } else if (args[i] === '--backup') {
      backup = true;
    } else if (args[i] === '--help') {
      console.log(`
Database Drop Script for Atomic

Usage: node scripts/drop-database.js [options]

Completely deletes the database file. The app will create a fresh database on next startup.

Options:
  --db <path>      Custom database path
  --force          Skip confirmation prompt
  --backup         Create backup before deleting
  --help           Show this help message

Examples:
  node scripts/drop-database.js
  node scripts/drop-database.js --backup
  node scripts/drop-database.js --force
      `);
      return;
    }
  }

  // Use default path if not specified
  if (!dbPath) {
    dbPath = getDefaultDbPath();
  }

  console.log('Database Drop Script for Atomic\n');
  console.log(`Database: ${dbPath}\n`);

  // Check if database exists
  if (!fs.existsSync(dbPath)) {
    console.log('✓ Database file does not exist (already dropped or never created).');
    return;
  }

  // Show file info
  const stats = fs.statSync(dbPath);
  console.log(`File size: ${formatFileSize(stats.size)}`);
  console.log(`Last modified: ${stats.mtime.toLocaleString()}`);

  // Confirmation prompt
  if (!force) {
    console.log('\n⚠️  WARNING: This will PERMANENTLY DELETE the database file!');
    console.log('All atoms, tags, settings, and embeddings will be lost.\n');
    const confirmed = await promptConfirmation('Type \'yes\' to continue: ');

    if (!confirmed) {
      console.log('\nCancelled.');
      return;
    }
  }

  // Create backup if requested
  if (backup) {
    createBackup(dbPath);
  }

  // Delete the database file
  console.log('\nDeleting database...');
  fs.unlinkSync(dbPath);
  console.log('✓ Database deleted successfully!\n');
  console.log('Start the Atomic app to create a fresh database.');
}

main().catch(console.error);
