## Description

<!-- Provide a brief description of your changes -->

## Related Issue

<!-- Link to the related issue(s) -->
Closes #

## Type of Change

<!-- Mark the relevant option with an "x" -->

- [ ] `feat`: New feature (minor version bump)
- [ ] `fix`: Bug fix (patch version bump)
- [ ] `docs`: Documentation only changes
- [ ] `chore`: Maintenance tasks, dependency updates
- [ ] `refactor`: Code refactoring without functional changes
- [ ] `test`: Adding or updating tests
- [ ] `ci`: CI/CD changes
- [ ] `perf`: Performance improvements
- [ ] `build`: Build system changes
- [ ] `style`: Code style/formatting changes

## PR Title Format

**IMPORTANT**: Since we use squash and merge, your PR title will become the commit message. Please ensure your PR title follows the [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<optional-scope>): <description>
```

### Examples:
- `feat(operators): add new string comparison operator`
- `fix(wasm): correct memory allocation bug`
- `docs: update API examples in README`
- `chore(deps): update rust dependencies`

For breaking changes, use `!` after the type/scope or include `BREAKING CHANGE:` in the PR description:
- `feat(api)!: redesign evaluation API`

## Testing

<!-- Describe the testing you've performed -->

- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Manual testing performed
- [ ] All tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Clippy checks pass (`cargo clippy -- -D warnings`)
- [ ] WASM builds successfully (if applicable)

## Breaking Changes

<!-- If this introduces breaking changes, describe them here -->

- [ ] This PR includes breaking changes
- [ ] Documentation has been updated to reflect breaking changes
- [ ] Migration guide included (if needed)

## Additional Notes

<!-- Any additional information, context, or screenshots -->
