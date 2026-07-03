#!/bin/bash

cd "$(dirname "$0")"


# абсолютный путь к проекту, где лежат библиотеки
PROJECT_DIR="/Users/YOUR_USERNAME/Documents/YOUR_PROJECT_DIR"

# экспортируем для python-скрипта
export PROJECT_DIR="$PROJECT_DIR"

# логирование
echo "Starting Plugin. Project Dir: $PROJECT_DIR" > plugin.log

# поиск python в проекте (venv) или системный
if [ -f "$PROJECT_DIR/env/bin/python3" ]; then
    PYTHON_EXEC="$PROJECT_DIR/env/bin/python3"
elif [ -f "$PROJECT_DIR/.venv/bin/python3" ]; then
    PYTHON_EXEC="$PROJECT_DIR/.venv/bin/python3"
else
    PYTHON_EXEC="python3"
fi

echo "Using Python: $PYTHON_EXEC" >> plugin.log

"$PYTHON_EXEC" main.py "$@" 2>> plugin.log
