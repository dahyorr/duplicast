#!/bin/bash
# Logging System Demo Script

echo "================================================"
echo "Duplicast Logging System Demo"
echo "================================================"
echo ""

echo "1. INFO Level (Default) - Production mode"
echo "   Shows: Server events, connections, stream lifecycle"
echo "   Command: RUST_LOG=info cargo run"
echo ""

echo "2. DEBUG Level - Development mode"
echo "   Shows: All INFO + detailed debugging information"
echo "   Command: RUST_LOG=debug cargo run"
echo ""

echo "3. TRACE Level - Deep debugging"
echo "   Shows: All DEBUG + very detailed traces"
echo "   Command: RUST_LOG=trace cargo run"
echo ""

echo "4. ERROR Only - Production (quiet)"
echo "   Shows: Only critical errors"
echo "   Command: RUST_LOG=error cargo run"
echo ""

echo "5. Module-Specific Debugging"
echo "   Shows: Debug for specific modules only"
echo "   Command: RUST_LOG=duplicast_core::server=debug,duplicast_core::stream=trace cargo run"
echo ""

echo "================================================"
echo "Select a mode to run:"
echo "================================================"
echo "1) INFO (default)"
echo "2) DEBUG"
echo "3) TRACE"
echo "4) ERROR only"
echo "5) Server DEBUG + Stream TRACE"
echo "6) Exit"
echo ""

read -p "Enter choice [1-6]: " choice

case $choice in
    1)
        echo "Running with INFO level..."
        cd /home/dahyor/Projects/personal/duplicast/core
        RUST_LOG=info cargo run
        ;;
    2)
        echo "Running with DEBUG level..."
        cd /home/dahyor/Projects/personal/duplicast/core
        RUST_LOG=debug cargo run
        ;;
    3)
        echo "Running with TRACE level..."
        cd /home/dahyor/Projects/personal/duplicast/core
        RUST_LOG=trace cargo run
        ;;
    4)
        echo "Running with ERROR only..."
        cd /home/dahyor/Projects/personal/duplicast/core
        RUST_LOG=error cargo run
        ;;
    5)
        echo "Running with Server DEBUG + Stream TRACE..."
        cd /home/dahyor/Projects/personal/duplicast/core
        RUST_LOG=duplicast_core::server=debug,duplicast_core::stream=trace cargo run
        ;;
    6)
        echo "Exiting..."
        exit 0
        ;;
    *)
        echo "Invalid choice. Running with INFO level..."
        cd /home/dahyor/Projects/personal/duplicast/core
        RUST_LOG=info cargo run
        ;;
esac
