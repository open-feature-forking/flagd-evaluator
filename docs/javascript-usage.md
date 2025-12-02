# JavaScript/WASM Usage

This document describes how to build and use the flagd-evaluator as a JavaScript/WASM library.

## Building

Install wasm-pack if not already installed:

```bash
cargo install wasm-pack
```

Build for your target environment:

### For Web (browser with ES modules)
```bash
wasm-pack build --target web --features js --no-default-features
```

### For Node.js
```bash
wasm-pack build --target nodejs --features js --no-default-features
```

### For Bundlers (webpack, rollup, etc.)
```bash
wasm-pack build --target bundler --features js --no-default-features
```

The build output will be in the `pkg/` directory.

## Usage

### Browser (ES Modules)

```html
<script type="module">
    import init, { evaluate, FlagdEvaluator } from './pkg/flagd_evaluator.js';

    async function main() {
        await init();

        // Simple function API - returns a JavaScript object directly (no JSON.parse needed!)
        const result = evaluate('{"==": [1, 1]}', '{}');
        console.log('Success:', result.success); // true
        console.log('Result:', result.result);   // true

        // Class-based API
        const evaluator = new FlagdEvaluator();
        const result2 = evaluator.evaluate('{"var": "name"}', '{"name": "Alice"}');
        console.log('Name:', result2.result); // "Alice"
    }

    main();
</script>
```

### Node.js

```javascript
const { evaluate, FlagdEvaluator } = require('./pkg/flagd_evaluator.js');

// Simple function API - returns a JavaScript object directly
const result = evaluate('{"==": [1, 1]}', '{}');
console.log(result.success); // true
console.log(result.result);  // true

// Class-based API
const evaluator = new FlagdEvaluator();
const result2 = evaluator.evaluate('{"var": "user.id"}', '{"user": {"id": "123"}}');
console.log(result2.result); // "123"
```

### TypeScript

The functions return typed objects directly:

```typescript
import init, { evaluate, FlagdEvaluator } from './pkg/flagd_evaluator';

interface EvaluationResponse {
    success: boolean;
    result: any | null;
    error: string | null;
}

async function main() {
    await init();

    // No JSON.parse needed - the result is already a JavaScript object
    const result: EvaluationResponse = evaluate('{"==": [1, 1]}', '{}');
    
    if (result.success) {
        console.log('Result:', result.result);
    } else {
        console.error('Error:', result.error);
    }
}

main();
```

## Response Format

The `evaluate` function returns a JavaScript object with the following structure:

```typescript
{
    success: boolean;  // Whether the evaluation succeeded
    result: any;       // The evaluation result (null if error)
    error: string;     // Error message (null if success)
}
```

## Supported Operators

All standard JSON Logic operators plus custom operators:
- `fractional`: For A/B testing and gradual rollouts
- `starts_with`: String prefix matching
- `ends_with`: String suffix matching
- `sem_ver`: Semantic version comparison

## Key Differences from Chicory Build

The JavaScript/WASM build differs from the Chicory (Java) build:

| Feature | Chicory Build | JavaScript Build |
|---------|--------------|------------------|
| Memory Management | Manual (alloc/dealloc) | Automatic (wasm-bindgen) |
| API Style | Raw pointers, packed u64 | Native JavaScript objects |
| Return Type | JSON string | JavaScript object |
| Target | Pure WASM runtimes | Browser/Node.js |
| Bindings | Custom C-style exports | wasm-bindgen + serde-wasm-bindgen |

Both builds compile from the same core evaluation logic and support the same operators.

## Publishing to npm

After building with wasm-pack, you can publish to npm:

```bash
cd pkg
npm publish
```

The generated `package.json` will include appropriate metadata.
