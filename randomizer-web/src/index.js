import React from 'react';
import { render } from 'react-dom';
import App from './app'
import { ConsoleInterface } from 'randomizer-client';

/* Initialize console interface wasm module */
ConsoleInterface.init();

render(<App />, document.getElementById('app-root'));