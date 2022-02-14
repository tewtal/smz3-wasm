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
      },
      {
        test: /\.tsx?$/,
        use: 'ts-loader',
        exclude: /node_modules/,
      },
    ]
  },
  resolve: {
    extensions: ['.tsx', '.ts', '.js'],
  },
  devServer: {
    port: 4321
  },
  plugins: [new HtmlWebpackPlugin({
    title: 'Randomizer Web Tests',
    template: 'index.html'
  })],
};