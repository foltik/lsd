const config = {
  plugins: [
    require.resolve("prettier-plugin-jinja-template"),
    require.resolve("prettier-plugin-tailwindcss"),
  ],
  overrides: [
    {
      files: ["*.tera.html"],
      options: {
        parser: "jinja-template",
      },
    },
  ],
};

module.exports = config;
