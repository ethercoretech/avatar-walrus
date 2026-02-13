const fs = require('fs');
const path = require('path');
const solc = require('solc');

const contractPath = path.join(__dirname, 'MiniUSDT.sol');
const source = fs.readFileSync(contractPath, 'utf8');

function findImports(importPath) {
    if (importPath.startsWith('@openzeppelin/')) {
        const fullPath = path.join(__dirname, 'node_modules', importPath);
        return { contents: fs.readFileSync(fullPath, 'utf8') };
    }
    const fullPath = path.join(__dirname, importPath);
    if (fs.existsSync(fullPath)) {
        return { contents: fs.readFileSync(fullPath, 'utf8') };
    }
    return { error: 'File not found: ' + importPath };
}

const input = {
    language: 'Solidity',
    sources: {
        'MiniUSDT.sol': {
            content: source
        }
    },
    settings: {
        outputSelection: {
            '*': {
                '*': ['abi', 'evm.bytecode']
            }
        }
    }
};

const output = JSON.parse(solc.compile(JSON.stringify(input), { import: findImports }));

if (output.errors) {
    output.errors.forEach(err => {
        console.error(err.formattedMessage);
    });
    process.exit(1);
}

const contract = output.contracts['MiniUSDT.sol']['MiniUSDT'];
const bytecode = contract.evm.bytecode.object;
const abi = contract.abi;

const outputPath = path.join(__dirname, 'MiniUSDT.json');
fs.writeFileSync(outputPath, JSON.stringify({
    abi: abi,
    bytecode: '0x' + bytecode
}, null, 2));

console.log('Contract compiled successfully!');
console.log('Bytecode length:', bytecode.length);
console.log('Output saved to:', outputPath);
