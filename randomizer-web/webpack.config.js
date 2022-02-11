const HtmlWebpackPlugin = require('html-webpack-plugin');
const path = require('path');

module.exports = {
  experiments: {
    asyncWebAssembly: true
  },
  entry: './src/index',
  mode: 'development',
  output: {
    filename: '[name].bundle.js'
  },
  module: {
    rules: [
      {
        test: /\.js$/,
        exclude: /node_modules/,
        use: 'babel-loader'
      }
    ]
  },
  devServer: {
    port: 4321
  },
  plugins: [new HtmlWebpackPlugin({
    title: 'Randomizer Web Tests',
    template: 'index.html'
  })],
};