# Contributing to rxRust

We welcome and appreciate all contributions to rxRust! Whether it's reporting bugs, suggesting new features, improving documentation, or submitting code, your help makes this project better.

## Roadmap & Missing Features

We maintain a list of features and operators that are either planned or would be valuable additions to rxRust. Please refer to `missing_features.md` in the project root for the current roadmap and a list of operators that need implementation. If you're looking for a good place to start, picking an operator from this list is an excellent way to contribute!

For guidance on how to implement custom operators to contribute to rxRust, please see [Step 5.2: Contributing Operators to rxRust](advanced/custom_operators.md#52-contributing-operators-to-rxrust) in the Custom Operators guide.

## Development Workflow

1.  **Fork and Clone**: Start by forking the `rxRust` repository on GitHub and cloning your fork locally.
    ```bash
    git clone https://github.com/your-username/rxRust.git
    cd rxRust
    ```

2.  **Create a Branch**: Create a new branch for your feature or bug fix.
    ```bash
    git checkout -b feature/my-new-operator
    # or
    git checkout -b bugfix/fix-issue-123
    ```

3.  **Code Style**: rxRust enforces consistent code style and formatting.
    *   **Formatting**: We use `rustfmt`. Before committing, run:
        ```bash
        cargo +nightly fmt
        ```
    *   **Linting**: We use `clippy` for linting. Always run `clippy` to catch common mistakes and improve your code.
        ```bash
        cargo clippy --all-targets --all-features
        ```

4.  **Testing**:
    *   **Unit Tests**: Run all unit tests to ensure your changes haven't introduced regressions:
        ```bash
        cargo test --all-targets --all-features
        ```
    *   **Examples**: If you're adding a significant new feature or a complex operator, or if existing examples don't cover your changes well, please include an example in the `examples/` directory or update relevant examples in the `guide/` to demonstrate its usage. We welcome examples, but there's no need to create a dedicated example for every single operator.
    *   **Documentation Examples**: Ensure any code examples in the documentation (`guide/`) compile and work correctly.

## Submitting a Pull Request (PR)

Once you're satisfied with your changes and all tests pass:

1.  **Commit Your Changes**: Write clear, concise commit messages.
    ```bash
    git add .
    git commit -m "feat: implement my new amazing operator"
    ```

2.  **Push to Your Fork**:
    ```bash
    git push origin feature/my-new-operator
    ```

3.  **Open a Pull Request**: Go to the original `rxRust` repository on GitHub and open a new Pull Request from your forked branch to the `main` branch.

    *   **Describe Your Changes**: Provide a clear description of your changes, including:
        *   What problem does this PR solve?
        *   How does it solve it?
        *   Any relevant context, design decisions, or trade-offs.
        *   Link to any open issues this PR addresses (e.g., "Closes #123").
    *   **Code Review**: Be prepared for feedback and discussions during the code review process. We might suggest changes to improve readability, performance, or adherence to project conventions.

Thank you for contributing to rxRust! Your efforts help us build a more robust and complete reactive programming library for Rust.